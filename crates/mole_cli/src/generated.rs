use serde_json::{json, Value};
use std::{collections::BTreeMap, fs, path::Path, time::UNIX_EPOCH};

use crate::{
    generated_artifact_statuses, GeneratedArtifactStatus, GeneratedCommand, SCHEMA_VERSION,
};

struct ArtifactGroup {
    id: &'static str,
    name: &'static str,
    generator: &'static str,
    recommended_command: &'static str,
    inputs: &'static [&'static str],
    outputs: &'static [&'static str],
}

pub(crate) fn generated_report(root: &Path, command: &GeneratedCommand) -> Value {
    match command {
        GeneratedCommand::Check => generated_check_report(root),
    }
}

fn generated_check_report(root: &Path) -> Value {
    let status_by_path = generated_artifact_statuses(root)
        .into_iter()
        .map(|status| (status.path.clone(), status))
        .collect::<BTreeMap<_, _>>();
    let groups = generated_artifact_groups()
        .into_iter()
        .map(|group| artifact_group_report(root, group, &status_by_path))
        .collect::<Vec<_>>();

    let summary = json!({
        "total_groups": groups.len(),
        "ok_groups": count_groups(&groups, "ok"),
        "missing_input_groups": count_nonempty_array(&groups, "missing_inputs"),
        "missing_output_groups": count_nonempty_array(&groups, "missing_outputs"),
        "stale_groups": count_groups(&groups, "stale"),
        "dirty_output_groups": count_nonempty_array(&groups, "dirty_outputs"),
    });
    let ok = groups
        .iter()
        .all(|group| group.get("ok").and_then(Value::as_bool).unwrap_or(false));

    json!({
        "schema_version": SCHEMA_VERSION,
        "command": "generated check",
        "project_root": root.display().to_string(),
        "mutated": false,
        "ok": ok,
        "summary": summary,
        "artifact_groups": groups,
    })
}

fn artifact_group_report(
    root: &Path,
    group: ArtifactGroup,
    status_by_path: &BTreeMap<String, GeneratedArtifactStatus>,
) -> Value {
    let dependencies = std::iter::once(group.generator)
        .chain(group.inputs.iter().copied())
        .collect::<Vec<_>>();
    let missing_inputs = dependencies
        .iter()
        .copied()
        .filter(|relative| !root.join(relative).exists())
        .collect::<Vec<_>>();
    let missing_outputs = group
        .outputs
        .iter()
        .copied()
        .filter(|relative| !root.join(relative).exists())
        .collect::<Vec<_>>();
    let dirty_outputs = group
        .outputs
        .iter()
        .copied()
        .filter(|relative| {
            status_by_path
                .get(*relative)
                .is_some_and(|status| status.dirty)
        })
        .collect::<Vec<_>>();
    let output_mtimes = group
        .outputs
        .iter()
        .copied()
        .filter_map(|relative| mtime_millis(root.join(relative).as_path()))
        .collect::<Vec<_>>();
    let input_mtimes = dependencies
        .iter()
        .copied()
        .filter_map(|relative| {
            mtime_millis(root.join(relative).as_path()).map(|mtime| (relative, mtime))
        })
        .collect::<Vec<_>>();
    let oldest_output_mtime_ms = output_mtimes.iter().copied().min();
    let newest_input_mtime_ms = input_mtimes.iter().map(|(_relative, mtime)| *mtime).max();
    let newer_inputs = if missing_outputs.is_empty() {
        oldest_output_mtime_ms
            .map(|oldest_output| {
                input_mtimes
                    .iter()
                    .filter(|(_relative, mtime)| *mtime > oldest_output)
                    .map(|(relative, _mtime)| *relative)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let stale = !newer_inputs.is_empty();
    let ok = missing_inputs.is_empty()
        && missing_outputs.is_empty()
        && dirty_outputs.is_empty()
        && !stale;

    json!({
        "id": group.id,
        "name": group.name,
        "generator": group.generator,
        "recommended_command": group.recommended_command,
        "inputs": group.inputs,
        "outputs": group.outputs,
        "missing_inputs": missing_inputs,
        "missing_outputs": missing_outputs,
        "dirty_outputs": dirty_outputs,
        "newer_inputs": newer_inputs,
        "oldest_output_mtime_ms": oldest_output_mtime_ms,
        "newest_input_mtime_ms": newest_input_mtime_ms,
        "stale": stale,
        "ok": ok,
    })
}

fn mtime_millis(path: &Path) -> Option<u128> {
    fs::metadata(path)
        .ok()?
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis())
}

fn count_groups(groups: &[Value], field: &str) -> usize {
    groups
        .iter()
        .filter(|group| group.get(field).and_then(Value::as_bool).unwrap_or(false))
        .count()
}

fn count_nonempty_array(groups: &[Value], field: &str) -> usize {
    groups
        .iter()
        .filter(|group| {
            group
                .get(field)
                .and_then(Value::as_array)
                .is_some_and(|items| !items.is_empty())
        })
        .count()
}

fn generated_artifact_groups() -> Vec<ArtifactGroup> {
    vec![
        ArtifactGroup {
            id: "value_sheets",
            name: "Value Sheets",
            generator: "tools/generate_value_sheets.py",
            recommended_command: ".venv\\Scripts\\python.exe tools\\generate_value_sheets.py",
            inputs: &[
                "resources/melee/extracted/plco_common_data.json",
                "resources/melee/extracted/captain_falcon_profile.json",
            ],
            outputs: &[
                "docs/state_graphs/value_sheets/global_common_values.json",
                "docs/state_graphs/value_sheets/captain_falcon_values.json",
            ],
        },
        ArtifactGroup {
            id: "value_parity_diff_report",
            name: "Value Parity Diff Report",
            generator: "tools/export_parity_diff_report.py",
            recommended_command: ".venv\\Scripts\\python.exe tools\\export_parity_diff_report.py",
            inputs: &[
                "docs/state_graphs/value_sheets/global_common_values.json",
                "docs/state_graphs/value_sheets/captain_falcon_values.json",
                "tools/state_graph_viewer.py",
            ],
            outputs: &[
                "docs/state_graphs/parity_reports/value_diffs.json",
                "docs/state_graphs/parity_reports/value_diffs.md",
            ],
        },
        ArtifactGroup {
            id: "falcon_ecb_generated_tables",
            name: "Falcon ECB Generated Tables",
            generator: "tools/generate_falcon_ecb_rust.py",
            recommended_command: ".venv\\Scripts\\python.exe tools\\generate_falcon_ecb_rust.py",
            inputs: &[
                "resources/melee/extracted/captain_falcon_action_ecb_samples.json",
                "tools/falcon_ecb_mapping.py",
            ],
            outputs: &[
                "crates/mole_core/src/generated/falcon_ecb.rs",
                "docs/state_graphs/parity_reports/falcon_ecb_coverage.json",
            ],
        },
        ArtifactGroup {
            id: "runtime_source_frame_data",
            name: "Runtime Source Frame Data",
            generator: "crates/mole_cli/src/frame_data.rs",
            recommended_command: "cargo run -p mole_cli -- frame-data export-runtime --all-states --character dolphin_mole --output crates/mole_runtime/src/generated/source_frame_data.rs --write --json",
            inputs: &[
                "resources/melee/frame_data/dolphin_mole/source_manifest.json",
                "resources/melee/raw/PlCaAJ.dat",
                "crates/mole_frame_data/src/lib.rs",
            ],
            outputs: &[
                "crates/mole_runtime/src/generated/source_frame_data.rs",
                "crates/mole_runtime/src/generated/source_frame_data/source_manifest.json",
                "crates/mole_runtime/src/generated/source_frame_data/source_frame_capsules.bin",
                "crates/mole_runtime/src/generated/source_frame_data/attack_air_n.figatree.bin",
            ],
        },
    ]
}
