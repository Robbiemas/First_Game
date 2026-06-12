use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

use serde_json::{json, Value};

use crate::{PackageCommand, SCHEMA_VERSION};

const PACKAGE_NAME: &str = "MoleGame-FriendPlaytest";
const LOCAL_INTERNET_PACKAGE_NAME: &str = "MoleGame-LocalInternetPlaytest";
const PACKAGE_SCRIPT: &str = "tools/package_friend_playtest.ps1";

pub(crate) fn package_report(root: &Path, command: &PackageCommand) -> Value {
    match command {
        PackageCommand::FriendPlaytest { verify, dry_run } => {
            playtest_package_report(root, *verify, *dry_run, PackageKind::Friend)
        }
        PackageCommand::LocalInternetPlaytest { verify, dry_run } => {
            playtest_package_report(root, *verify, *dry_run, PackageKind::LocalInternet)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PackageKind {
    Friend,
    LocalInternet,
}

impl PackageKind {
    const fn command(self) -> &'static str {
        match self {
            Self::Friend => "package friend-playtest",
            Self::LocalInternet => "package local-internet-playtest",
        }
    }

    const fn playtest_exe_name(self) -> &'static str {
        match self {
            Self::Friend => "MoleGame-FriendPlaytest.exe",
            Self::LocalInternet => "MoleGame-LocalInternetPlaytest.exe",
        }
    }
}

fn playtest_package_report(root: &Path, verify: bool, dry_run: bool, kind: PackageKind) -> Value {
    let script = root.join(PACKAGE_SCRIPT);
    let package_folder = root.join("dist").join(PACKAGE_NAME);
    let package_zip = root.join("dist").join(format!("{PACKAGE_NAME}.zip"));
    let dist_exe = root.join("dist").join(format!("{PACKAGE_NAME}.exe"));
    let local_internet_dist_exe = root
        .join("dist")
        .join(format!("{LOCAL_INTERNET_PACKAGE_NAME}.exe"));
    let playtest_exe = root.join("playtest").join(kind.playtest_exe_name());
    let build_command = vec![
        "powershell".to_string(),
        "-NoProfile".to_string(),
        "-ExecutionPolicy".to_string(),
        "Bypass".to_string(),
        "-File".to_string(),
        PACKAGE_SCRIPT.to_string(),
    ];

    let mut report = json!({
        "schema_version": SCHEMA_VERSION,
        "command": kind.command(),
        "project_root": root.display().to_string(),
        "mutated": !dry_run,
        "dry_run": dry_run,
        "verify": verify,
        "build_command": build_command,
        "artifacts": {
            "package_script": script.display().to_string(),
            "package_folder": package_folder.display().to_string(),
            "package_zip": package_zip.display().to_string(),
            "dist_exe": dist_exe.display().to_string(),
            "local_internet_dist_exe": local_internet_dist_exe.display().to_string(),
            "playtest_exe": playtest_exe.display().to_string(),
        },
    });

    if dry_run {
        report["ok"] = json!(true);
        report["steps"] = json!([]);
        return report;
    }

    let mut steps = Vec::new();
    let build = run_process(
        root,
        "powershell",
        &[
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
            PACKAGE_SCRIPT,
        ],
        &[],
        &[],
    );
    let build_ok = step_ok(&build);
    steps.push(build);

    let artifact_checks = required_artifact_checks(&[
        package_folder.as_path(),
        package_zip.as_path(),
        dist_exe.as_path(),
        local_internet_dist_exe.as_path(),
        playtest_exe.as_path(),
    ]);
    let artifacts_ok = artifact_checks
        .as_array()
        .is_some_and(|checks| checks.iter().all(|check| check["exists"] == true));
    steps.push(json!({
        "name": "artifact checks",
        "ok": artifacts_ok,
        "checks": artifact_checks,
    }));

    let mut verify_ok = true;
    if verify && build_ok && artifacts_ok {
        let verify_steps = verify_playtest_exe(&playtest_exe);
        verify_ok = verify_steps
            .iter()
            .all(|step| step.get("ok").and_then(Value::as_bool).unwrap_or(false));
        steps.extend(verify_steps);
    }

    report["ok"] = json!(build_ok && artifacts_ok && verify_ok);
    report["steps"] = json!(steps);
    report
}

fn verify_playtest_exe(playtest_exe: &Path) -> Vec<Value> {
    let mut steps = Vec::new();
    let extract = run_process(
        playtest_exe.parent().unwrap_or_else(|| Path::new(".")),
        playtest_exe.to_string_lossy().as_ref(),
        &[],
        &[("MOLE_PLAYTEST_EXTRACT_ONLY", "1")],
        &[],
    );
    let extract_ok = step_ok(&extract);
    steps.push(with_step_name("one-file extraction", extract));

    let package_root = local_app_data_dir().join(PACKAGE_NAME);
    let required = [
        "mole_runtime.exe",
        "SDL3.dll",
        "DolphinMole",
        "Run Mole Game.cmd",
        "Run Mole Game Vanilla No UCF.cmd",
        "Run Solo Internet Host.cmd",
        "Run Headless Internet Peer.cmd",
        "Run Local Internet Playtest.cmd",
        "Run Local Practice.cmd",
        "Run Local Practice Vanilla No UCF.cmd",
        "Check WUP Adapter.cmd",
        "Monitor WUP Input.cmd",
        "README.md",
    ];
    let extracted_checks = required
        .iter()
        .map(|relative| {
            let path = package_root.join(relative);
            json!({
                "path": path.display().to_string(),
                "exists": path.exists(),
            })
        })
        .collect::<Vec<_>>();
    let extracted_ok = extract_ok
        && extracted_checks
            .iter()
            .all(|check| check["exists"].as_bool().unwrap_or(false));
    steps.push(json!({
        "name": "extracted package checks",
        "ok": extracted_ok,
        "package_root": package_root.display().to_string(),
        "checks": extracted_checks,
    }));

    if extracted_ok {
        let runtime = package_root.join("mole_runtime.exe");
        let path_env = packaged_path_env(&package_root);
        let runtime_smoke = run_process(
            &package_root,
            runtime.to_string_lossy().as_ref(),
            &["--friend-connect", "--frames", "2", "--input-trace"],
            &[("PATH", path_env.as_str())],
            &["MOLE_ASSET_ROOT"],
        );
        steps.push(with_step_name(
            "packaged runtime startup without repo asset root",
            runtime_smoke,
        ));
    }

    steps
}

fn required_artifact_checks(paths: &[&Path]) -> Value {
    paths
        .iter()
        .map(|path| {
            json!({
                "path": path.display().to_string(),
                "exists": path.exists(),
            })
        })
        .collect::<Vec<_>>()
        .into()
}

fn run_process(
    cwd: &Path,
    program: &str,
    args: &[&str],
    envs: &[(&str, &str)],
    env_remove: &[&str],
) -> Value {
    let mut command = Command::new(program);
    command.current_dir(cwd).args(args);
    for (key, value) in envs {
        command.env(key, value);
    }
    for key in env_remove {
        command.env_remove(key);
    }
    match command.output() {
        Ok(output) => {
            let ok = output.status.success();
            json!({
                "program": program,
                "args": args,
                "cwd": cwd.display().to_string(),
                "ok": ok,
                "status_code": output.status.code(),
                "stdout_tail": output_tail(&output.stdout),
                "stderr_tail": output_tail(&output.stderr),
            })
        }
        Err(error) => json!({
            "program": program,
            "args": args,
            "cwd": cwd.display().to_string(),
            "ok": false,
            "error": error.to_string(),
        }),
    }
}

fn with_step_name(name: &str, mut step: Value) -> Value {
    step["name"] = json!(name);
    step
}

fn step_ok(step: &Value) -> bool {
    step.get("ok").and_then(Value::as_bool).unwrap_or(false)
}

fn output_tail(bytes: &[u8]) -> String {
    const MAX_CHARS: usize = 4000;
    let text = String::from_utf8_lossy(bytes);
    let chars = text.chars().collect::<Vec<_>>();
    let start = chars.len().saturating_sub(MAX_CHARS);
    chars[start..].iter().collect::<String>()
}

fn local_app_data_dir() -> PathBuf {
    env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(env::temp_dir)
}

fn packaged_path_env(package_root: &Path) -> String {
    let current = env::var_os("PATH")
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_default();
    if current.is_empty() {
        package_root.display().to_string()
    } else {
        format!("{};{current}", package_root.display())
    }
}
