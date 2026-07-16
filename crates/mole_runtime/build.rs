use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let workspace = manifest_dir.parent().unwrap().parent().unwrap();
    let roots = [
        workspace.join("crates/mole_core/src"),
        workspace.join("crates/mole_input/src"),
        workspace.join("crates/mole_rollback/src"),
        workspace.join("crates/mole_runtime/src"),
        workspace.join("crates/mole_transport/src"),
    ];
    let mut files = Vec::new();
    for root in &roots {
        collect_rust_files(root, &mut files);
        println!("cargo:rerun-if-changed={}", root.display());
    }
    files.push(manifest_dir.join("build.rs"));
    files.sort();

    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for path in files {
        let relative = path.strip_prefix(workspace).unwrap_or(&path);
        mix(
            &mut hash,
            relative.to_string_lossy().replace('\\', "/").as_bytes(),
        );
        mix(&mut hash, &fs::read(&path).unwrap());
    }
    println!("cargo:rustc-env=MOLE_BUILD_FINGERPRINT={hash:016x}");
}

fn collect_rust_files(root: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_rust_files(&path, files);
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            files.push(path);
        }
    }
}

fn mix(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
}
