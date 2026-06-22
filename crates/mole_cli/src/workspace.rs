use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use serde_json::{json, Value};

use crate::{WorkspaceCommand, SCHEMA_VERSION};

const REQUIRED_LAUNCHERS: &[&str] = &[
    "execs/Play Slippi Replay.cmd",
    "execs/Run SDL3 Runtime.cmd",
    "execs/Open Dev Tool.cmd",
    "execs/Clean Local Outputs.cmd",
];

pub(crate) fn workspace_report(root: &Path, command: &WorkspaceCommand) -> Value {
    match command {
        WorkspaceCommand::Health => workspace_health_report(root),
    }
}

fn workspace_health_report(root: &Path) -> Value {
    let checks = json!({
        "sdl3_runtime": path_check(root, ".local/SDL3/lib/x64/SDL3.dll"),
        "release_runtime": path_check(root, "target/release/mole_runtime.exe"),
        "replay_source": path_check(root, "replays/Game_20260530T214929.slp"),
        "divergence_log": path_check(root, "debug/slippi/runtime-divergence.latest.json"),
        "required_launchers": required_launcher_checks(root),
        "generated_source_frame_data": generated_source_frame_data_check(root),
    });
    let ok = checks["sdl3_runtime"]["present"].as_bool().unwrap_or(false)
        && checks["release_runtime"]["present"]
            .as_bool()
            .unwrap_or(false)
        && checks["replay_source"]["present"]
            .as_bool()
            .unwrap_or(false)
        && checks["required_launchers"]
            .as_array()
            .map(|launchers| {
                launchers.iter().all(|launcher| {
                    launcher["present"].as_bool().unwrap_or(false)
                        && launcher["tracked"].as_bool().unwrap_or(false)
                })
            })
            .unwrap_or(false)
        && checks["generated_source_frame_data"]["extra_loose_figatree_sidecars"]
            .as_array()
            .map(Vec::is_empty)
            .unwrap_or(false);

    json!({
        "schema_version": SCHEMA_VERSION,
        "command": "workspace health",
        "project_root": root.display().to_string(),
        "ok": ok,
        "checks": checks,
    })
}

fn path_check(root: &Path, relative_path: &str) -> Value {
    let path = root.join(relative_path);
    json!({
        "path": relative_path,
        "present": path.exists(),
    })
}

fn required_launcher_checks(root: &Path) -> Vec<Value> {
    REQUIRED_LAUNCHERS
        .iter()
        .map(|relative_path| {
            let path = root.join(relative_path);
            json!({
                "path": relative_path,
                "present": path.exists(),
                "tracked": git_tracks_path(root, relative_path),
            })
        })
        .collect()
}

fn git_tracks_path(root: &Path, relative_path: &str) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("ls-files")
        .arg("--error-unmatch")
        .arg(relative_path)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn generated_source_frame_data_check(root: &Path) -> Value {
    let dir = root.join("crates/mole_runtime/src/generated/source_frame_data");
    let mut extra_loose_figatree_sidecars = Vec::new();
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) == Some("bin")
                && path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .is_some_and(|name| name.ends_with(".figatree.bin"))
            {
                extra_loose_figatree_sidecars.push(file_name(path));
            }
        }
    }
    extra_loose_figatree_sidecars.sort();

    json!({
        "directory": "crates/mole_runtime/src/generated/source_frame_data",
        "present": dir.exists(),
        "source_frame_capsules": path_check(root, "crates/mole_runtime/src/generated/source_frame_data/source_frame_capsules.bin"),
        "source_manifest": path_check(root, "crates/mole_runtime/src/generated/source_frame_data/source_manifest.json"),
        "figatree_bundle": path_check(root, "crates/mole_runtime/src/generated/source_frame_data/source_figatree_bundle.bin"),
        "extra_loose_figatree_sidecars": extra_loose_figatree_sidecars,
    })
}

fn file_name(path: PathBuf) -> String {
    path.file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_string()
}
