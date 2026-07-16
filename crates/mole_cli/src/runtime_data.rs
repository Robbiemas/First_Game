use std::{fs, path::Path};

use serde_json::{json, Value};

use crate::{RuntimeDataCommand, SCHEMA_VERSION};

const GAP_LEDGER_PATH: &str = "docs/research/decomp-parity-gap-ledger.json";
const RUNTIME_SOURCE_FRAME_DATA_PATH: &str =
    "crates/mole_runtime/src/generated/source_frame_data.rs";

const RAW_SOURCE_ARTIFACTS: &[&str] = &[
    "resources/melee/raw/PlCa.dat",
    "resources/melee/raw/PlCaAJ.dat",
    "resources/melee/raw/PlCaNr.dat",
];

const ARTIFACTS: &[ArtifactSpec] = &[
    ArtifactSpec {
        path: "resources/melee/extracted/captain_falcon_action_animation_table.json",
        role: "debug_inspection",
        runtime_input: false,
    },
    ArtifactSpec {
        path: "resources/melee/extracted/captain_falcon_action_ecb_samples.json",
        role: "debug_sample_cache",
        runtime_input: false,
    },
    ArtifactSpec {
        path: "resources/melee/frame_data/dolphin_mole/source_manifest.json",
        role: "resolved_source_manifest",
        runtime_input: false,
    },
    ArtifactSpec {
        path: "crates/mole_runtime/src/generated/source_frame_data.rs",
        role: "compact_runtime",
        runtime_input: true,
    },
    ArtifactSpec {
        path: "crates/mole_runtime/src/generated/source_frame_data/source_frame_capsules.bin",
        role: "compact_runtime",
        runtime_input: true,
    },
    ArtifactSpec {
        path: "crates/mole_runtime/src/generated/source_frame_data/source_figatree_bundle.bin",
        role: "compact_runtime",
        runtime_input: true,
    },
    ArtifactSpec {
        path: "crates/mole_runtime/src/generated/source_frame_data/source_manifest.json",
        role: "compact_runtime",
        runtime_input: true,
    },
    ArtifactSpec {
        path: "crates/mole_core/src/generated/falcon_ecb.rs",
        role: "legacy_expanded_runtime",
        runtime_input: true,
    },
];

#[derive(Debug, Clone, Copy)]
struct ArtifactSpec {
    path: &'static str,
    role: &'static str,
    runtime_input: bool,
}

pub(crate) fn runtime_data_report(root: &Path, command: &RuntimeDataCommand) -> Value {
    match command {
        RuntimeDataCommand::GapLedger => gap_ledger(root),
        RuntimeDataCommand::IdentityReport => identity_report(root),
        RuntimeDataCommand::SizeReport => size_report(root),
    }
}

fn gap_ledger(root: &Path) -> Value {
    let path = root.join(GAP_LEDGER_PATH);
    let mut ledger = match fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
    {
        Some(value) => value,
        None => {
            return json!({
                "schema_version": SCHEMA_VERSION,
                "command": "runtime-data gap-ledger",
                "project_root": root.display().to_string(),
                "source_path": GAP_LEDGER_PATH,
                "ok": false,
                "error": format!("missing or invalid {GAP_LEDGER_PATH}"),
                "summary": gap_summary(&[]),
                "gaps": [],
            });
        }
    };

    let gaps = ledger
        .get("gaps")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    ledger["schema_version"] = json!(SCHEMA_VERSION);
    ledger["command"] = json!("runtime-data gap-ledger");
    ledger["project_root"] = json!(root.display().to_string());
    ledger["source_path"] = json!(GAP_LEDGER_PATH);
    ledger["ok"] = json!(true);
    ledger["summary"] = gap_summary(&gaps);
    ledger
}

fn gap_summary(gaps: &[Value]) -> Value {
    let mut by_subsystem = serde_json::Map::new();
    let mut open = 0_u64;
    for gap in gaps {
        if gap.get("status").and_then(Value::as_str) == Some("open") {
            open += 1;
        }
        if let Some(subsystem) = gap.get("subsystem").and_then(Value::as_str) {
            let count = by_subsystem
                .get(subsystem)
                .and_then(Value::as_u64)
                .unwrap_or(0)
                + 1;
            by_subsystem.insert(subsystem.to_string(), json!(count));
        }
    }

    json!({
        "total": gaps.len(),
        "open": open,
        "by_subsystem": by_subsystem,
    })
}

fn identity_report(root: &Path) -> Value {
    let path = root.join(RUNTIME_SOURCE_FRAME_DATA_PATH);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => {
            return json!({
                "schema_version": SCHEMA_VERSION,
                "command": "runtime-data identity-report",
                "project_root": root.display().to_string(),
                "source_path": RUNTIME_SOURCE_FRAME_DATA_PATH,
                "ok": false,
                "error": format!("failed to read {RUNTIME_SOURCE_FRAME_DATA_PATH}: {error}"),
                "summary": identity_summary(&[]),
                "bindings": [],
            });
        }
    };

    let bindings = parse_runtime_action_bindings(&text);
    let binding_values = bindings
        .iter()
        .map(|binding| {
            json!({
                "source_action_key": binding.source_action_key,
                "motion_state": binding.motion_state,
                "melee_motion_state_id": binding.melee_motion_state_id,
                "source_action_table_index": binding.source_action_table_index,
                "identity_status": binding.identity_status(),
            })
        })
        .collect::<Vec<_>>();

    json!({
        "schema_version": SCHEMA_VERSION,
        "command": "runtime-data identity-report",
        "project_root": root.display().to_string(),
        "source_path": RUNTIME_SOURCE_FRAME_DATA_PATH,
        "ok": true,
        "identity_contract": {
            "melee_motion_state_id": "Melee runtime/decomp ftCo/ftCa motion-state identity when this baked binding has one.",
            "source_action_table_index": "Extracted Fighter_WaitAnimData action-table index used to locate source animation/action payloads.",
            "source_action_key": "Stable extracted action key; do not derive source identity from Rust enum discriminants or Slippi row labels.",
        },
        "summary": identity_summary(&bindings),
        "bindings": binding_values,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeActionIdentity {
    melee_motion_state_id: Option<u16>,
    source_action_table_index: u16,
    source_action_key: String,
    motion_state: Option<String>,
}

impl RuntimeActionIdentity {
    fn identity_status(&self) -> &'static str {
        match self.melee_motion_state_id {
            None => "source_only",
            Some(id) if id == self.source_action_table_index => "same_numeric_value",
            Some(_) => "distinct",
        }
    }
}

fn identity_summary(bindings: &[RuntimeActionIdentity]) -> Value {
    let runtime_motion_bindings = bindings
        .iter()
        .filter(|binding| binding.melee_motion_state_id.is_some())
        .count();
    let source_only_bindings = bindings.len().saturating_sub(runtime_motion_bindings);
    let distinct_identity_bindings = bindings
        .iter()
        .filter(|binding| binding.identity_status() == "distinct")
        .count();
    let same_numeric_identity_bindings = bindings
        .iter()
        .filter(|binding| binding.identity_status() == "same_numeric_value")
        .count();

    json!({
        "total_bindings": bindings.len(),
        "runtime_motion_bindings": runtime_motion_bindings,
        "source_only_bindings": source_only_bindings,
        "distinct_identity_bindings": distinct_identity_bindings,
        "same_numeric_identity_bindings": same_numeric_identity_bindings,
    })
}

fn parse_runtime_action_bindings(text: &str) -> Vec<RuntimeActionIdentity> {
    text.lines()
        .filter(|line| line.contains("RuntimeActionBinding {"))
        .filter_map(parse_runtime_action_binding)
        .collect()
}

fn parse_runtime_action_binding(line: &str) -> Option<RuntimeActionIdentity> {
    let source_action_table_index = parse_u16_after(
        line,
        "source_action_table_index: SourceActionTableIndex::new(",
    )?;
    let source_action_key = parse_quoted_after(line, "source_action_key: \"")?;
    let melee_motion_state_id = if line.contains("melee_motion_state_id: None") {
        None
    } else {
        parse_u16_after(line, "melee_motion_state_id: Some(MeleeMotionStateId::new(")
    };
    let motion_state = if line.contains("motion_state: None") {
        None
    } else {
        parse_symbol_after(line, "motion_state: Some(MotionState::")
    };

    Some(RuntimeActionIdentity {
        melee_motion_state_id,
        source_action_table_index,
        source_action_key,
        motion_state,
    })
}

fn parse_u16_after(line: &str, marker: &str) -> Option<u16> {
    let rest = line.split_once(marker)?.1;
    let digits = rest
        .chars()
        .take_while(|character| character.is_ascii_digit())
        .collect::<String>();
    digits.parse().ok()
}

fn parse_quoted_after(line: &str, marker: &str) -> Option<String> {
    line.split_once(marker)?
        .1
        .split_once('"')
        .map(|(value, _)| value.to_string())
}

fn parse_symbol_after(line: &str, marker: &str) -> Option<String> {
    line.split_once(marker)?
        .1
        .split_once(')')
        .map(|(value, _)| value.to_string())
}

fn size_report(root: &Path) -> Value {
    let source_artifacts: Vec<Value> = RAW_SOURCE_ARTIFACTS
        .iter()
        .map(|path| artifact_value(root, path, "source_baseline", false, 0))
        .collect();
    let source_baseline_bytes = source_artifacts
        .iter()
        .filter_map(|artifact| artifact.get("bytes").and_then(Value::as_u64))
        .sum::<u64>();

    let mut warnings = Vec::new();
    let artifacts: Vec<Value> = ARTIFACTS
        .iter()
        .map(|spec| {
            let status = artifact_status(
                root,
                spec.path,
                spec.role,
                spec.runtime_input,
                source_baseline_bytes,
            );
            if status == "expanded_runtime_warning" {
                warnings.push(format!(
                    "{} is an expanded runtime artifact; replace with compact source-backed evaluation before treating it as parity-complete runtime data.",
                    spec.path
                ));
            }
            artifact_value(root, spec.path, spec.role, spec.runtime_input, source_baseline_bytes)
        })
        .collect();

    let runtime_bytes = artifacts
        .iter()
        .filter(|artifact| artifact["runtime_input"].as_bool().unwrap_or(false))
        .filter_map(|artifact| artifact.get("bytes").and_then(Value::as_u64))
        .sum::<u64>();
    let debug_bytes = artifacts
        .iter()
        .filter(|artifact| {
            artifact["role"]
                .as_str()
                .is_some_and(|role| role.starts_with("debug_"))
        })
        .filter_map(|artifact| artifact.get("bytes").and_then(Value::as_u64))
        .sum::<u64>();
    let middleware_bytes = artifacts
        .iter()
        .filter(|artifact| artifact["role"] == "resolved_source_manifest")
        .filter_map(|artifact| artifact.get("bytes").and_then(Value::as_u64))
        .sum::<u64>();
    let expanded_runtime_warnings = artifacts
        .iter()
        .filter(|artifact| artifact["status"] == "expanded_runtime_warning")
        .count();

    json!({
        "schema_version": SCHEMA_VERSION,
        "command": "runtime-data size-report",
        "project_root": root.display().to_string(),
        "ok": expanded_runtime_warnings == 0,
        "policy": {
            "normal_runtime_may_consume_debug_artifacts": false,
            "runtime_contract": "Normal play consumes compact baked runtime artifacts only; source/debug JSON may exist for extraction, inspection, and parity tests.",
            "editor_contract": "Devtool edits resolve through typed source/override artifacts before runtime generation, not by mutating extracted baselines or hand-authoring gameplay ECB boxes."
        },
        "source_baseline": {
            "role": "source_baseline",
            "bytes": source_baseline_bytes,
            "artifacts": source_artifacts,
        },
        "totals": {
            "runtime_bytes": runtime_bytes,
            "debug_bytes": debug_bytes,
            "middleware_bytes": middleware_bytes,
            "expanded_runtime_warnings": expanded_runtime_warnings,
        },
        "artifacts": artifacts,
        "warnings": warnings,
    })
}

fn artifact_value(
    root: &Path,
    relative_path: &str,
    role: &str,
    runtime_input: bool,
    source_baseline_bytes: u64,
) -> Value {
    let bytes = file_size(root, relative_path);
    let expansion_reasons = expansion_reasons(root, relative_path, role);
    let ratio = match (bytes, source_baseline_bytes) {
        (Some(bytes), baseline) if baseline > 0 => Some(bytes as f64 / baseline as f64),
        _ => None,
    };

    json!({
        "path": relative_path,
        "role": role,
        "runtime_input": runtime_input,
        "present": bytes.is_some(),
        "bytes": bytes.unwrap_or(0),
        "source_baseline_ratio": ratio,
        "expansion_reasons": expansion_reasons,
        "status": artifact_status(root, relative_path, role, runtime_input, source_baseline_bytes),
    })
}

fn artifact_status(
    root: &Path,
    relative_path: &str,
    role: &str,
    runtime_input: bool,
    source_baseline_bytes: u64,
) -> &'static str {
    let Some(bytes) = file_size(root, relative_path) else {
        return "missing";
    };
    if role == "legacy_expanded_runtime"
        && source_baseline_bytes > 0
        && (bytes > source_baseline_bytes
            || !expansion_reasons(root, relative_path, role).is_empty())
    {
        return "expanded_runtime_warning";
    }
    if runtime_input {
        "runtime_input"
    } else {
        "non_runtime_artifact"
    }
}

fn file_size(root: &Path, relative_path: &str) -> Option<u64> {
    fs::metadata(root.join(relative_path))
        .ok()
        .map(|metadata| metadata.len())
}

fn expansion_reasons(root: &Path, relative_path: &str, role: &str) -> Vec<&'static str> {
    if role != "legacy_expanded_runtime" {
        return Vec::new();
    }
    let text = fs::read_to_string(root.join(relative_path)).unwrap_or_default();
    let mut reasons = Vec::new();
    if text.contains("FALCON_SOURCE_ECB_ACTION_") {
        reasons.push("per_frame_source_ecb_samples");
    }
    if text.contains("FALCON_ECB_ACTION_") {
        reasons.push("per_frame_runtime_ecb_samples");
    }
    if text.contains("falcon_eval_source_ecb_from_jobj") {
        reasons.push("live_jobj_evaluator_embedded_in_legacy_artifact");
    }
    reasons
}
