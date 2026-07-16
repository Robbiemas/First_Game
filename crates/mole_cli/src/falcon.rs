use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use crate::{read_json, SCHEMA_VERSION};

const RUNTIME_SOURCE_MANIFEST: &str =
    "crates/mole_runtime/src/generated/source_frame_data/source_manifest.json";
const CURRENT_GRAPH: &str = "docs/state_graphs/mole_current_graph.json";
const CORE_STATE_RS: &str = "crates/mole_core/src/state.rs";
const CORE_SIM_RS: &str = "crates/mole_core/src/sim.rs";
const CORE_TESTS_RS: &str = "crates/mole_core/tests/core_contract.rs";
const RUNTIME_TESTS_RS: &str = "crates/mole_runtime/tests/runtime_contract.rs";

const AUTHORITATIVE_RENDER_KEYS: &[&str] = &[
    "EscapeAir",
    "Landing",
    "LandingAirN",
    "LandingAirF",
    "LandingAirB",
    "LandingAirHi",
    "LandingAirLw",
    "Attack100Start",
    "Attack100Loop",
    "Attack100End",
    "Catch",
    "CatchWait",
    "CatchAttack",
    "ThrowF",
    "ThrowB",
    "ThrowHi",
    "ThrowLw",
    "CapturePulledHi",
    "CaptureWaitHi",
    "CaptureDamageHi",
    "CapturePulledLw",
    "CaptureWaitLw",
    "CaptureDamageLw",
    "CaptureCut",
    "CaptureJump",
    "TCaptainThrowF",
    "TCaptainThrowB",
    "TCaptainThrowHi",
    "TCaptainThrowLw",
    "TCaptainSpecialHi",
];

const PRIORITY_GAMEPLAY_KEYS: &[&str] = &[
    "EscapeAir",
    "Landing",
    "LandingAirN",
    "LandingAirF",
    "LandingAirB",
    "LandingAirHi",
    "LandingAirLw",
    "Attack100Start",
    "Attack100Loop",
    "Attack100End",
    "AttackS3Hi",
    "AttackS3HiS",
    "AttackS3LwS",
    "AttackS3Lw",
    "AttackS4Hi",
    "AttackS4Lw",
    "DownDamageU",
    "DownDamageD",
    "DownFowardU",
    "DownFowardD",
    "DownBackU",
    "DownBackD",
    "DownSpotD",
    "PassiveWall",
    "PassiveWallJump",
    "PassiveCeil",
    "WallDamage",
    "StopWall",
    "StopCeil",
    "Rebound",
    "MissFoot",
    "CliffWait2",
    "FuraFura",
    "FuraSleepStart",
    "FuraSleepLoop",
    "FuraSleepEnd",
    "Ottotto",
    "OttottoWait",
    "AppealR",
    "AppealL",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FighterCoverageOptions {
    pub source_character: String,
    pub target_character: String,
}

impl FighterCoverageOptions {
    pub(crate) fn captain_dolphin_mole() -> Self {
        Self {
            source_character: "captain".to_string(),
            target_character: "dolphin_mole".to_string(),
        }
    }
}

pub(crate) fn fighter_coverage_report(root: &Path, options: &FighterCoverageOptions) -> Value {
    let mut errors = Vec::new();
    let extracted_action_table_path = extracted_action_table_path(&options.source_character);
    let full_source_manifest_path = format!(
        "resources/melee/frame_data/{}/source_manifest.json",
        options.target_character
    );
    let extracted = read_json_or_error(root, &extracted_action_table_path, &mut errors);
    let runtime_manifest = read_json_or_error(root, RUNTIME_SOURCE_MANIFEST, &mut errors);
    let full_manifest = read_json_or_error(root, &full_source_manifest_path, &mut errors);
    let graph = read_json_or_error(root, CURRENT_GRAPH, &mut errors);

    let extracted_actions = actions_from(&extracted);
    let runtime_actions = actions_from(&runtime_manifest);
    let runtime_keys = action_key_set(&runtime_actions);
    let runtime_motion_bound = runtime_actions
        .iter()
        .filter(|action| {
            action
                .get("runtime_motion_state")
                .and_then(Value::as_str)
                .is_some()
        })
        .filter_map(|action| action_summary(action, "runtime_motion_bound"))
        .collect::<Vec<_>>();
    let source_only_bound = runtime_actions
        .iter()
        .filter(|action| {
            action
                .get("runtime_motion_state")
                .is_some_and(Value::is_null)
        })
        .filter_map(|action| action_summary(action, "source_only_bound"))
        .collect::<Vec<_>>();

    let source_figatree_present = runtime_actions
        .iter()
        .filter_map(|action| action_summary(action, "source_figatree_present"))
        .collect::<Vec<_>>();
    let no_figatree_action_slot = full_manifest
        .get("skipped_source_actions")
        .and_then(Value::as_array)
        .map(|actions| {
            actions
                .iter()
                .map(|action| {
                    json!({
                        "action_state_id": action.get("action_state_id").cloned().unwrap_or(Value::Null),
                        "source_action_key": Value::Null,
                        "state": Value::Null,
                        "reason": action.get("reason").cloned().unwrap_or(Value::Null),
                        "category": "no_figatree_action_slot",
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let missing_runtime_manifest = extracted_actions
        .iter()
        .filter(|action| {
            action.get("status").and_then(Value::as_str) == Some("available_for_jobj_sampling")
        })
        .filter(|action| {
            action
                .get("figatree_root")
                .and_then(Value::as_str)
                .and_then(source_key_from_figatree_root)
                .is_some_and(|key| !runtime_keys.contains(key))
        })
        .filter_map(|action| {
            let key = action
                .get("figatree_root")
                .and_then(Value::as_str)
                .and_then(source_key_from_figatree_root)?;
            Some(json!({
                "action_state_id": action.get("action_state_id").cloned().unwrap_or(Value::Null),
                "source_action_key": key,
                "state": key,
                "category": "missing_runtime_manifest",
            }))
        })
        .collect::<Vec<_>>();

    let state_rs = read_text(root, CORE_STATE_RS, &mut errors);
    let sim_rs = read_text(root, CORE_SIM_RS, &mut errors);
    let core_tests = read_text(root, CORE_TESTS_RS, &mut errors);
    let runtime_tests = read_text(root, RUNTIME_TESTS_RS, &mut errors);
    let graph_nodes = graph_node_ids(&graph);

    let missing_rust_binding = runtime_actions
        .iter()
        .filter_map(|action| action.get("source_action_key").and_then(Value::as_str))
        .filter(|key| !state_rs.contains(&format!("\"{key}\"")))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|key| category_entry_for_key(key, "missing_rust_binding"))
        .collect::<Vec<_>>();

    let missing_state_graph_node = runtime_actions
        .iter()
        .filter_map(|action| action.get("state").and_then(Value::as_str))
        .filter(|state| !graph_nodes.contains(*state))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|state| category_entry_for_key(state, "missing_state_graph_node"))
        .collect::<Vec<_>>();

    let render_contract_text = format!("{runtime_tests}\n{core_tests}");
    let missing_render_contract = AUTHORITATIVE_RENDER_KEYS
        .iter()
        .copied()
        .filter(|key| runtime_keys.contains(*key))
        .filter(|key| !render_contract_text.contains(key))
        .map(|key| category_entry_for_key(key, "missing_render_contract"))
        .collect::<Vec<_>>();

    let gameplay_text = format!("{sim_rs}\n{core_tests}\n{runtime_tests}");
    let missing_gameplay_route = PRIORITY_GAMEPLAY_KEYS
        .iter()
        .copied()
        .filter(|key| runtime_keys.contains(*key))
        .filter(|key| !gameplay_text.contains(key))
        .map(|key| category_entry_for_key(key, "missing_gameplay_route"))
        .collect::<Vec<_>>();

    let category_counts = BTreeMap::from([
        ("source_figatree_present", source_figatree_present.len()),
        ("no_figatree_action_slot", no_figatree_action_slot.len()),
        ("runtime_motion_bound", runtime_motion_bound.len()),
        ("source_only_bound", source_only_bound.len()),
        ("missing_rust_binding", missing_rust_binding.len()),
        ("missing_state_graph_node", missing_state_graph_node.len()),
        ("missing_render_contract", missing_render_contract.len()),
        ("missing_gameplay_route", missing_gameplay_route.len()),
        ("missing_runtime_manifest", missing_runtime_manifest.len()),
    ]);
    let ok = missing_runtime_manifest.is_empty() && errors.is_empty();

    json!({
        "schema_version": SCHEMA_VERSION,
        "command": "fighter coverage",
        "ok": ok,
        "source_character": options.source_character,
        "target_character": options.target_character,
        "authoritative_render_note": "Yellow source wireframe, source hurtboxes, source hitboxes, and ECB/debug collision are authoritative. The blue mole sprite overlay is a non-authoritative debug layer.",
        "inputs": {
            "extracted_action_table": extracted_action_table_path,
            "runtime_source_manifest": RUNTIME_SOURCE_MANIFEST,
            "full_source_manifest": full_source_manifest_path,
            "current_graph": CURRENT_GRAPH,
        },
        "summary": {
            "extracted_action_slots": extracted_actions.len(),
            "source_figatree_present": source_figatree_present.len(),
            "no_figatree_action_slots": no_figatree_action_slot.len(),
            "runtime_motion_bound": runtime_motion_bound.len(),
            "source_only_bound": source_only_bound.len(),
            "missing_runtime_manifest": missing_runtime_manifest.len(),
        },
        "category_counts": category_counts,
        "categories": {
            "source_figatree_present": source_figatree_present,
            "no_figatree_action_slot": no_figatree_action_slot,
            "runtime_motion_bound": runtime_motion_bound,
            "source_only_bound": source_only_bound,
            "missing_rust_binding": missing_rust_binding,
            "missing_state_graph_node": missing_state_graph_node,
            "missing_render_contract": missing_render_contract,
            "missing_gameplay_route": missing_gameplay_route,
            "missing_runtime_manifest": missing_runtime_manifest,
        },
        "errors": errors,
    })
}

pub(crate) fn falcon_coverage_report(root: &Path) -> Value {
    fighter_coverage_report(root, &FighterCoverageOptions::captain_dolphin_mole())
}

fn extracted_action_table_path(source_character: &str) -> String {
    let extracted_name = match source_character {
        "captain" | "captain_falcon" => "captain_falcon".to_string(),
        other => other.replace('-', "_"),
    };
    format!("resources/melee/extracted/{extracted_name}_action_animation_table.json")
}

fn read_json_or_error(root: &Path, relative_path: &str, errors: &mut Vec<String>) -> Value {
    match read_json(root.join(relative_path)) {
        Ok(value) => value,
        Err(error) => {
            errors.push(format!("{relative_path}: {error}"));
            Value::Null
        }
    }
}

fn read_text(root: &Path, relative_path: &str, errors: &mut Vec<String>) -> String {
    match fs::read_to_string(root.join(relative_path)) {
        Ok(value) => value,
        Err(error) => {
            errors.push(format!("{relative_path}: {error}"));
            String::new()
        }
    }
}

fn actions_from(value: &Value) -> Vec<&Value> {
    let Some(actions) = value.get("actions") else {
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

fn action_key_set(actions: &[&Value]) -> BTreeSet<String> {
    actions
        .iter()
        .filter_map(|action| action.get("source_action_key").and_then(Value::as_str))
        .map(str::to_string)
        .collect()
}

fn graph_node_ids(graph: &Value) -> BTreeSet<String> {
    graph
        .get("nodes")
        .and_then(Value::as_array)
        .map(|nodes| {
            nodes
                .iter()
                .filter_map(|node| node.get("id").and_then(Value::as_str))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn action_summary(action: &Value, category: &str) -> Option<Value> {
    Some(json!({
        "action_state_id": action.get("action_state_id").cloned().unwrap_or(Value::Null),
        "source_action_key": action.get("source_action_key").cloned().unwrap_or(Value::Null),
        "state": action.get("state").cloned().unwrap_or(Value::Null),
        "runtime_motion_state": action.get("runtime_motion_state").cloned().unwrap_or(Value::Null),
        "total_frames": action.get("total_frames").cloned().unwrap_or(Value::Null),
        "category": category,
    }))
}

fn category_entry_for_key(key: &str, category: &str) -> Value {
    json!({
        "source_action_key": key,
        "state": key,
        "category": category,
    })
}

fn source_key_from_figatree_root(root: &str) -> Option<&str> {
    root.strip_prefix("PlyCaptain5K_Share_ACTION_")
        .and_then(|suffix| suffix.strip_suffix("_figatree"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falcon_coverage_reports_runtime_manifest_completeness() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("crate should be inside workspace");
        let report = fighter_coverage_report(root, &FighterCoverageOptions::captain_dolphin_mole());

        assert_eq!(report["command"], "fighter coverage");
        assert_eq!(report["source_character"], "captain");
        assert_eq!(report["target_character"], "dolphin_mole");
        assert_eq!(report["summary"]["source_figatree_present"], 275);
        assert_eq!(report["summary"]["no_figatree_action_slots"], 43);
        assert_eq!(report["summary"]["missing_runtime_manifest"], 0);
        assert_eq!(
            report["authoritative_render_note"],
            "Yellow source wireframe, source hurtboxes, source hitboxes, and ECB/debug collision are authoritative. The blue mole sprite overlay is a non-authoritative debug layer."
        );
    }

    #[test]
    fn falcon_coverage_tracks_expected_priority_gaps() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("crate should be inside workspace");
        let report = fighter_coverage_report(root, &FighterCoverageOptions::captain_dolphin_mole());

        let missing_gameplay = report["categories"]["missing_gameplay_route"]
            .as_array()
            .expect("missing gameplay category should be an array");
        assert!(missing_gameplay.iter().any(|entry| {
            entry["source_action_key"] == "CliffWait2"
                || entry["source_action_key"] == "PassiveWall"
                || entry["source_action_key"] == "AppealR"
        }));
        assert!(report["categories"]["source_only_bound"]
            .as_array()
            .expect("source-only category should be an array")
            .iter()
            .any(|entry| entry["source_action_key"] == "ThrowLw"));
    }
}
