use std::collections::BTreeMap;

use crate::MESSAGE_BOARD_PATH;

pub(crate) fn changed_paths_from_git_status(status: &str) -> Vec<String> {
    status
        .lines()
        .filter(|line| line.len() >= 4)
        .filter_map(|line| {
            let path = line[3..].trim();
            if path.is_empty() {
                None
            } else if let Some((_old, new)) = path.split_once(" -> ") {
                Some(new.to_string())
            } else {
                Some(path.to_string())
            }
        })
        .collect()
}

pub fn verification_plan_for_changed_paths(paths: &[String]) -> Vec<String> {
    verification_plan_with_reasons_for_changed_paths(paths).0
}

pub(crate) fn verification_plan_with_reasons_for_changed_paths(
    paths: &[String],
) -> (Vec<String>, BTreeMap<String, Vec<String>>) {
    let mut commands = Vec::<String>::new();
    let mut reasons = BTreeMap::<String, Vec<String>>::new();

    if paths.is_empty() {
        push_unique_command_with_reason(
            &mut commands,
            &mut reasons,
            "git diff --check",
            "No changed paths were reported; whitespace validation is the minimum safe check.",
        );
        return (commands, reasons);
    }

    let needs_workspace_cargo = paths.iter().any(|path| {
        path == "Cargo.toml"
            || path == "Cargo.lock"
            || path.starts_with("crates/mole_replay/")
            || path.starts_with("crates/mole_rollback/")
            || path.starts_with("crates/mole_transport/")
            || path.starts_with("crates/mole_signaling/")
    });
    if needs_workspace_cargo {
        let reason = paths
            .iter()
            .find(|path| *path == "Cargo.toml" || *path == "Cargo.lock")
            .map(|path| format!("Workspace Cargo manifest changed: {path}."))
            .unwrap_or_else(|| {
                "Workspace-level Rust crate changed; run the full workspace.".to_string()
            });
        push_unique_command_with_reason(
            &mut commands,
            &mut reasons,
            "cargo test --workspace",
            &reason,
        );
    } else {
        if paths.iter().any(|path| {
            path.starts_with("crates/mole_core/") || path.starts_with("resources/melee/extracted/")
        }) {
            push_unique_command_with_reason(
                &mut commands,
                &mut reasons,
                "cargo test -p mole_core",
                "Rust core or extracted Melee resource data changed.",
            );
        }
        if paths
            .iter()
            .any(|path| path.starts_with("crates/mole_input/"))
        {
            push_unique_command_with_reason(
                &mut commands,
                &mut reasons,
                "cargo test -p mole_input",
                "Input adapter or UCF preprocessing code changed.",
            );
        }
        if paths
            .iter()
            .any(|path| path.starts_with("crates/mole_runtime/"))
        {
            push_unique_command_with_reason(
                &mut commands,
                &mut reasons,
                "cargo test -p mole_runtime",
                "SDL runtime, WUP bridge, rendering, or diagnostics code changed.",
            );
        }
        if paths
            .iter()
            .any(|path| path.starts_with("crates/mole_cli/") || path == MESSAGE_BOARD_PATH)
        {
            push_unique_command_with_reason(
                &mut commands,
                &mut reasons,
                "cargo test -p mole_cli",
                "Mole CLI code or request-board workflow changed.",
            );
        }
    }
    if paths.iter().any(|path| {
        path.starts_with("tools/")
            || path.starts_with("docs/state_graphs/")
            || path.starts_with("config/state_graph_layout")
    }) {
        push_unique_command_with_reason(
            &mut commands,
            &mut reasons,
            ".venv\\Scripts\\python.exe -m pytest tests\\test_value_sheets.py tests\\test_state_graph_viewer.py tests\\test_parity_diff_report.py tests\\test_generate_falcon_ecb_rust.py tests\\test_extract_melee_resources.py tests\\test_slippi_replay_tools.py -q",
            "State graph, value sheet, extractor, Slippi, or dev-tool artifact changed.",
        );
        push_unique_command_with_reason(
            &mut commands,
            &mut reasons,
            ".venv\\Scripts\\python.exe tools\\state_graph_viewer.py --check",
            "State graph viewer or graph data changed.",
        );
    }
    if paths.iter().any(|path| {
        path.starts_with("execs/") || path == "README.md" || path.starts_with("docs/architecture/")
    }) {
        push_unique_command_with_reason(
            &mut commands,
            &mut reasons,
            ".venv\\Scripts\\python.exe -m pytest tests\\test_launch_inputs.py -q",
            "Launcher, README, or architecture-facing launch instructions changed.",
        );
    }
    if commands.is_empty() {
        push_unique_command_with_reason(
            &mut commands,
            &mut reasons,
            "cargo test -p mole_cli",
            "Fallback verification for files without a more specific command mapping.",
        );
    }
    push_unique_command_with_reason(
        &mut commands,
        &mut reasons,
        "git diff --check",
        "Always validate whitespace and conflict markers before claiming completion.",
    );
    (commands, reasons)
}

fn push_unique_command_with_reason(
    commands: &mut Vec<String>,
    reasons: &mut BTreeMap<String, Vec<String>>,
    command: &str,
    reason: &str,
) {
    if !commands.iter().any(|existing| existing == command) {
        commands.push(command.to_string());
    }
    let entries = reasons.entry(command.to_string()).or_default();
    if !entries.iter().any(|existing| existing == reason) {
        entries.push(reason.to_string());
    }
}
