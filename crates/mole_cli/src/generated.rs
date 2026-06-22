use serde_json::{json, Value};
use std::{collections::BTreeMap, fs, path::Path, time::UNIX_EPOCH};

use crate::{
    action_motion_tables, fighter_common, generated_artifact_statuses, ledger_map, stage_assets,
    value_sheets, GeneratedArtifactStatus, GeneratedCommand, SCHEMA_VERSION,
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
        GeneratedCommand::WriteValueSheets { write } => {
            value_sheets::write_value_sheets_report(root, *write)
        }
        GeneratedCommand::WriteStageAsset { stage, write } => {
            stage_assets::write_stage_asset_report(root, stage, *write)
        }
        GeneratedCommand::WriteLedgerMap { write } => {
            ledger_map::write_ledger_map_report(root, *write)
        }
        GeneratedCommand::WriteActionMotionTables { write } => {
            action_motion_tables::write_action_motion_tables_report(root, *write)
        }
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
            generator: value_sheets::VALUE_SHEETS_GENERATOR,
            recommended_command: value_sheets::VALUE_SHEETS_COMMAND,
            inputs: value_sheets::VALUE_SHEETS_INPUTS,
            outputs: value_sheets::VALUE_SHEET_OUTPUTS,
        },
        ArtifactGroup {
            id: "battlefield_stage_asset",
            name: "Battlefield Stage Asset",
            generator: stage_assets::STAGE_ASSETS_GENERATOR,
            recommended_command: stage_assets::STAGE_ASSETS_COMMAND,
            inputs: stage_assets::STAGE_ASSET_INPUTS,
            outputs: stage_assets::STAGE_ASSET_OUTPUTS,
        },
        ArtifactGroup {
            id: "fighter_common_accessories",
            name: "Fighter Common Accessories",
            generator: fighter_common::FIGHTER_COMMON_GENERATOR,
            recommended_command: fighter_common::FIGHTER_COMMON_COMMAND,
            inputs: fighter_common::FIGHTER_COMMON_INPUTS,
            outputs: fighter_common::FIGHTER_COMMON_OUTPUTS,
        },
        ArtifactGroup {
            id: "parity_ledger_map",
            name: "Parity Ledger Map",
            generator: ledger_map::LEDGER_MAP_GENERATOR,
            recommended_command: ledger_map::LEDGER_MAP_COMMAND,
            inputs: ledger_map::LEDGER_MAP_INPUTS,
            outputs: ledger_map::LEDGER_MAP_OUTPUTS,
        },
        ArtifactGroup {
            id: "action_motion_tables",
            name: "Action / Motion Tables",
            generator: action_motion_tables::ACTION_MOTION_TABLES_GENERATOR,
            recommended_command: action_motion_tables::ACTION_MOTION_TABLES_COMMAND,
            inputs: action_motion_tables::ACTION_MOTION_TABLES_INPUTS,
            outputs: action_motion_tables::ACTION_MOTION_TABLES_OUTPUTS,
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
                "tools/rust_literals.py",
            ],
            outputs: &[
                "crates/mole_core/src/generated/falcon_ecb.rs",
                "docs/state_graphs/parity_reports/falcon_ecb_coverage.json",
            ],
        },
        ArtifactGroup {
            id: "source_root_motion_generated_tables",
            name: "Source Root Motion Generated Tables",
            generator: "tools/generate_source_root_motion_rust.py",
            recommended_command: ".venv\\Scripts\\python.exe tools\\generate_source_root_motion_rust.py",
            inputs: &[
                "resources/melee/frame_data/dolphin_mole/source_manifest.json",
                "resources/melee/raw/PlCaAJ.dat",
                "crates/mole_cli/src/frame_data.rs",
                "crates/mole_frame_data/src/lib.rs",
                "tools/rust_literals.py",
            ],
            outputs: &["crates/mole_core/src/generated/source_root_motion.rs"],
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
                "crates/mole_runtime/src/generated/source_frame_data/source_frame_capsules.bin",
            ],
        },
    ]
}
