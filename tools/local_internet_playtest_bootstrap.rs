use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

const PACKAGE_NAME: &str = "MoleGame-FriendPlaytest";
const PACKAGE_ZIP: &[u8] = include_bytes!("../dist/MoleGame-FriendPlaytest.zip");
const LAUNCHER: &str = "Run Local Internet Playtest.cmd";

fn main() {
    if let Err(error) = run() {
        eprintln!("Mole Game local internet playtest launcher failed: {error}");
        eprintln!("Press Enter to close.");
        let mut line = String::new();
        let _ = io::stdin().read_line(&mut line);
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let base_dir = local_app_data_dir().join(PACKAGE_NAME);
    let zip_path = local_app_data_dir().join(format!("{PACKAGE_NAME}.zip"));

    remove_existing_package(&base_dir)?;
    fs::create_dir_all(&base_dir)?;
    fs::write(&zip_path, PACKAGE_ZIP)?;

    let extract_command = format!(
        "Expand-Archive -LiteralPath {} -DestinationPath {} -Force",
        powershell_path_literal(&zip_path),
        powershell_path_literal(&base_dir)
    );
    let status = Command::new("powershell")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &extract_command,
        ])
        .status()?;

    if !status.success() {
        return Err(format!("PowerShell extraction failed with status {status}").into());
    }

    let launcher = base_dir.join(LAUNCHER);
    if !launcher.is_file() {
        return Err(format!(
            "launcher not found after extraction: {}",
            launcher.display()
        )
        .into());
    }

    if extract_only() {
        println!(
            "Extracted Mole Game local internet playtest package to {}",
            base_dir.display()
        );
        return Ok(());
    }

    Command::new("cmd")
        .args(["/C", "start", "", launcher.to_string_lossy().as_ref()])
        .current_dir(&base_dir)
        .spawn()?;

    Ok(())
}

fn local_app_data_dir() -> PathBuf {
    env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(env::temp_dir)
}

fn powershell_path_literal(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\'', "''");
    format!("'{text}'")
}

fn extract_only() -> bool {
    env::args().any(|arg| arg == "--extract-only")
        || env::var_os("MOLE_PLAYTEST_EXTRACT_ONLY")
            .is_some_and(|value| value.to_string_lossy() == "1")
}

fn remove_existing_package(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if !path.exists() {
        return Ok(());
    }

    let base = local_app_data_dir().canonicalize()?;
    let existing = path.canonicalize()?;
    if !existing.starts_with(&base) {
        return Err(format!(
            "refusing to remove path outside local app data: {}",
            path.display()
        )
        .into());
    }

    fs::remove_dir_all(existing)?;
    Ok(())
}
