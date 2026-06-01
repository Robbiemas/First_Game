use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

const PACKAGE_NAME: &str = "MoleGame-FriendPlaytest";
const PACKAGE_ZIP: &[u8] = include_bytes!("../dist/MoleGame-FriendPlaytest.zip");

fn main() {
    if let Err(error) = run() {
        eprintln!("Mole Game playtest launcher failed: {error}");
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

    let status = Command::new("powershell")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            "Expand-Archive -LiteralPath $args[0] -DestinationPath $args[1] -Force",
        ])
        .arg(&zip_path)
        .arg(&base_dir)
        .status()?;

    if !status.success() {
        return Err(format!("PowerShell extraction failed with status {status}").into());
    }

    let launcher = base_dir.join("Run Mole Game.cmd");
    if !launcher.is_file() {
        return Err(format!(
            "launcher not found after extraction: {}",
            launcher.display()
        )
        .into());
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
