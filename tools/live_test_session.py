from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

try:
    from tools.process_lock import acquire_instance_lock
except ModuleNotFoundError:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
    from tools.process_lock import acquire_instance_lock

WATCHED_SUFFIXES = {".py", ".json", ".rs", ".toml", ".lock"}
WATCHED_NAMES = {"requirements.txt"}
IGNORED_DIRS = {
    ".git",
    ".local",
    ".pytest_cache",
    ".venv",
    "__pycache__",
    "logs",
    "target",
    "venv",
}
ALREADY_RUNNING_EXIT_CODE = 2


def wup_target_dir(root: Path) -> Path:
    return root.resolve() / "target-wup"


def wup_runtime_exe(root: Path) -> Path:
    exe = "mole_runtime.exe" if os.name == "nt" else "mole_runtime"
    return wup_target_dir(root) / "debug" / exe


def live_session_lock_path(root: Path) -> Path:
    return root.resolve() / "logs" / "live-test-session.lock"


def should_watch_file(path: Path, root: Path) -> bool:
    rel = _relative_path(path, root)
    if _has_ignored_part(rel):
        return False
    return path.name in WATCHED_NAMES or path.suffix.lower() in WATCHED_SUFFIXES


def collect_snapshot(root: Path) -> dict[str, tuple[int, int]]:
    snapshot = {}
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [
            name
            for name in dirnames
            if not _is_ignored_dir(name)
        ]
        current_dir = Path(dirpath)
        for filename in filenames:
            path = current_dir / filename
            if not should_watch_file(path, root):
                continue
            try:
                stat = path.stat()
            except FileNotFoundError:
                continue
            snapshot[_snapshot_key(path, root)] = (stat.st_mtime_ns, stat.st_size)
    return snapshot


def diff_snapshots(
    before: dict[str, tuple[int, int]],
    after: dict[str, tuple[int, int]],
) -> list[str]:
    changed = []
    for key in sorted(set(before) | set(after)):
        if before.get(key) != after.get(key):
            changed.append(key)
    return changed


def build_game_env(root: Path, base_env: dict[str, str] | None = None) -> dict[str, str]:
    root = root.resolve()
    env = dict(os.environ if base_env is None else base_env)
    env["MOLE_DEBUG_INPUTS"] = "1"
    env["MOLE_DEBUG_INPUT_LOG"] = "1"
    env["MOLE_LIVE_SESSION_CHILD"] = "1"
    env["MOLE_RUNTIME_EXE"] = str(wup_runtime_exe(root))
    env["SDL_JOYSTICK_HIDAPI"] = "1"
    env["SDL_JOYSTICK_HIDAPI_GAMECUBE"] = "1"

    cargo_bin = Path.home() / ".cargo" / "bin"
    existing_path = env.get("PATH", "")
    env["PATH"] = f"{cargo_bin}{os.pathsep}{existing_path}" if existing_path else str(cargo_bin)
    env.setdefault("PYTHONUNBUFFERED", "1")
    return env


def latest_log(log_dir: Path, prefix: str) -> Path | None:
    matches = list(log_dir.glob(f"{prefix}*"))
    if not matches:
        return None
    return max(matches, key=lambda path: path.stat().st_mtime_ns)


def run_live_session(
    root: Path,
    *,
    python: Path | None = None,
    poll_interval: float = 0.5,
    debounce_seconds: float = 0.75,
    build_rust: bool = True,
    auto_restart: bool = False,
    show_state_graph: bool = True,
) -> int:
    root = root.resolve()
    if os.environ.get("MOLE_LIVE_SESSION_CHILD") == "1":
        print("Refusing to start a nested Mole live test session.")
        return ALREADY_RUNNING_EXIT_CODE

    session_lock = acquire_instance_lock(live_session_lock_path(root))
    if session_lock is None:
        print("A Mole live test session is already running for this project.")
        print("Close the existing game window before launching another session.")
        return ALREADY_RUNNING_EXIT_CODE

    with session_lock:
        return _run_live_session_unlocked(
            root,
            python=python,
            poll_interval=poll_interval,
            debounce_seconds=debounce_seconds,
            build_rust=build_rust,
            auto_restart=auto_restart,
            show_state_graph=show_state_graph,
        )


def _run_live_session_unlocked(
    root: Path,
    *,
    python: Path | None = None,
    poll_interval: float = 0.5,
    debounce_seconds: float = 0.75,
    build_rust: bool = True,
    auto_restart: bool = False,
    show_state_graph: bool = True,
) -> int:
    python = python or default_python(root)
    env = build_game_env(root)

    if build_rust:
        build_wup_runtime(root, env)

    before = collect_snapshot(root)
    state_graph_process = (
        start_state_graph_viewer(root, python, env) if show_state_graph else None
    )
    process = start_game(root, python, env)
    last_change_at = None
    pending_changes: list[str] = []

    print("Live test session is watching for project changes.")
    if auto_restart:
        print("Auto-restart is enabled; source changes will restart the game.")
    else:
        print("Auto-restart is off; close and relaunch when you are ready to test edits.")
    print("Press Ctrl+C in this window to stop the session.")

    try:
        while True:
            exit_code = process.poll() if process is not None else None
            if exit_code is not None:
                process = None
                if state_graph_process is not None:
                    stop_game(state_graph_process)
                    state_graph_process = None
                if exit_code == 0:
                    print("Game closed normally; ending live session.")
                    return 0
                print(f"Game exited with code {exit_code}; ending live session.")
                report_latest_logs(root)
                return exit_code

            time.sleep(poll_interval)
            after = collect_snapshot(root)
            changed = diff_snapshots(before, after)
            if changed:
                before = after
                pending_changes = changed
                print("Change detected:")
                for item in changed[:8]:
                    print(f"  {item}")
                if len(changed) > 8:
                    print(f"  ...and {len(changed) - 8} more")
                if not auto_restart:
                    pending_changes = []
                    last_change_at = None
                    continue
                last_change_at = time.monotonic()

            if last_change_at is None:
                continue
            if time.monotonic() - last_change_at < debounce_seconds:
                continue

            if build_rust and any(Path(item).suffix.lower() == ".rs" for item in pending_changes):
                build_wup_runtime(root, env)
            if process is not None:
                stop_game(process)
            process = start_game(root, python, env)
            pending_changes = []
            last_change_at = None
    except KeyboardInterrupt:
        print("\nStopping live test session.")
        if process is not None:
            stop_game(process)
        if state_graph_process is not None:
            stop_game(state_graph_process)
        return 130


def default_python(root: Path) -> Path:
    exe = "python.exe" if os.name == "nt" else "python"
    return root / ".venv" / ("Scripts" if os.name == "nt" else "bin") / exe


def start_game(root: Path, python: Path, env: dict[str, str]) -> subprocess.Popen:
    game = root / "RealMainFile.py"
    print(f"Starting Mole Game with debug logging: {game}")
    return subprocess.Popen([str(python), str(game)], cwd=root, env=env)


def start_state_graph_viewer(
    root: Path,
    python: Path,
    env: dict[str, str],
) -> subprocess.Popen | None:
    root = root.resolve()
    viewer = root / "tools" / "state_graph_viewer.py"
    if not viewer.exists():
        return None
    print(f"Opening state transition graph viewer: {viewer}")
    return subprocess.Popen([str(python), str(viewer)], cwd=root, env=env)


def stop_game(process: subprocess.Popen, timeout_seconds: float = 3.0) -> None:
    if process.poll() is not None:
        return
    print("Restarting game process...")
    if os.name == "nt":
        subprocess.run(
            ["taskkill", "/PID", str(process.pid), "/T", "/F"],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        try:
            process.wait(timeout=timeout_seconds)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=timeout_seconds)
        return

    process.terminate()
    try:
        process.wait(timeout=timeout_seconds)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=timeout_seconds)


def build_wup_runtime(root: Path, env: dict[str, str]) -> None:
    cargo = shutil.which("cargo", path=env.get("PATH"))
    if cargo is None:
        print("Cargo was not found; skipping WUP runtime rebuild.")
        return
    print("Building WUP runtime helper...")
    result = subprocess.run(
        [
            cargo,
            "build",
            "-q",
            "-p",
            "mole_runtime",
            "--features",
            "wup",
            "--target-dir",
            str(wup_target_dir(root)),
        ],
        cwd=root,
        env=env,
    )
    if result.returncode != 0:
        print(f"WUP runtime build failed with code {result.returncode}.")


def report_latest_logs(root: Path) -> None:
    log_dir = root / "logs"
    crash = latest_log(log_dir, "crash-")
    input_log = latest_log(log_dir, "input-debug-")
    if crash is not None:
        print(f"Latest crash log: {crash}")
    if input_log is not None:
        print(f"Latest input log: {input_log}")


def check_setup(root: Path, python: Path | None = None) -> int:
    python = python or default_python(root)
    game = root / "RealMainFile.py"
    missing = [
        str(path)
        for path in (python, game)
        if not path.exists()
    ]
    if missing:
        print("Live test session check failed. Missing:")
        for path in missing:
            print(f"  {path}")
        return 1
    watched_count = len(collect_snapshot(root))
    print(f"Live test session check passed. Watching {watched_count} files.")
    return 0


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Launch Mole Game for live testing.")
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--python", type=Path, default=None)
    parser.add_argument("--poll", type=float, default=0.5)
    parser.add_argument("--debounce", type=float, default=0.75)
    parser.add_argument("--no-rust-build", action="store_true")
    parser.add_argument("--auto-restart", action="store_true")
    parser.add_argument(
        "--no-state-graph",
        dest="state_graph",
        action="store_false",
        default=True,
        help="Do not open the side-by-side state transition graph viewer.",
    )
    parser.add_argument("--check", action="store_true")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    if args.check:
        return check_setup(args.root, args.python)
    return run_live_session(
        args.root,
        python=args.python,
        poll_interval=args.poll,
        debounce_seconds=args.debounce,
        build_rust=not args.no_rust_build,
        auto_restart=args.auto_restart,
        show_state_graph=args.state_graph,
    )


def _relative_path(path: Path, root: Path) -> Path:
    try:
        return path.resolve().relative_to(root.resolve())
    except ValueError:
        return path


def _snapshot_key(path: Path, root: Path) -> str:
    return _relative_path(path, root).as_posix()


def _has_ignored_part(path: Path) -> bool:
    return any(_is_ignored_dir(part) for part in path.parts[:-1])


def _is_ignored_dir(name: str) -> bool:
    lower = name.lower()
    return lower in IGNORED_DIRS or lower.startswith("target-")


if __name__ == "__main__":
    raise SystemExit(main())
