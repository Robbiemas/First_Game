import os
import time
from pathlib import Path

from tools import live_test_session


def test_should_watch_game_source_config_and_rust_files(tmp_path):
    assert live_test_session.should_watch_file(tmp_path / "Characters.py", tmp_path)
    assert live_test_session.should_watch_file(tmp_path / "config" / "gamecube_calibration.json", tmp_path)
    assert live_test_session.should_watch_file(tmp_path / "crates" / "mole_core" / "src" / "lib.rs", tmp_path)
    assert live_test_session.should_watch_file(tmp_path / "Cargo.toml", tmp_path)


def test_should_ignore_generated_outputs_logs_and_virtualenv(tmp_path):
    ignored = [
        tmp_path / ".git" / "index",
        tmp_path / ".venv" / "Scripts" / "python.exe",
        tmp_path / "venv" / "Scripts" / "python.exe",
        tmp_path / "__pycache__" / "Characters.cpython.pyc",
        tmp_path / "logs" / "input-debug.jsonl",
        tmp_path / "target" / "debug" / "mole_runtime.exe",
        tmp_path / "target-codex" / "debug" / "mole_runtime.exe",
        tmp_path / "target-parity123" / "debug" / "mole_runtime.exe",
    ]

    for path in ignored:
        assert not live_test_session.should_watch_file(path, tmp_path)


def test_snapshot_diff_reports_modified_added_and_removed_files(tmp_path):
    watched = tmp_path / "ChooseAction.py"
    watched.write_text("before", encoding="utf-8")
    before = live_test_session.collect_snapshot(tmp_path)

    time.sleep(0.01)
    watched.write_text("after", encoding="utf-8")
    added = tmp_path / "Characters.py"
    added.write_text("new", encoding="utf-8")
    removed = tmp_path / "config" / "gamecube_calibration.json"
    removed.parent.mkdir()
    removed.write_text("old", encoding="utf-8")
    middle = live_test_session.collect_snapshot(tmp_path)

    removed.unlink()
    after = live_test_session.collect_snapshot(tmp_path)

    middle_changes = live_test_session.diff_snapshots(before, middle)
    after_changes = live_test_session.diff_snapshots(middle, after)

    assert "ChooseAction.py" in middle_changes
    assert "Characters.py" in middle_changes
    assert "config/gamecube_calibration.json" in after_changes


def test_build_game_env_forces_debug_overlay_and_input_log(tmp_path):
    env = live_test_session.build_game_env(tmp_path, {"PATH": "base-path"})

    assert env["MOLE_DEBUG_INPUTS"] == "1"
    assert env["MOLE_DEBUG_INPUT_LOG"] == "1"
    assert env["MOLE_LIVE_SESSION_CHILD"] == "1"
    assert env["MOLE_RUNTIME_EXE"] == str(
        tmp_path / "target-wup" / "debug" / "mole_runtime.exe"
    )
    assert env["SDL_JOYSTICK_HIDAPI"] == "1"
    assert env["SDL_JOYSTICK_HIDAPI_GAMECUBE"] == "1"
    assert env["PATH"].endswith("base-path")


def test_build_wup_runtime_uses_dedicated_target_dir(monkeypatch, tmp_path):
    calls = []

    def fake_which(name, path=None):
        assert name == "cargo"
        return "cargo.exe"

    def fake_run(command, **kwargs):
        calls.append((command, kwargs))

        class Result:
            returncode = 0

        return Result()

    monkeypatch.setattr(live_test_session.shutil, "which", fake_which)
    monkeypatch.setattr(live_test_session.subprocess, "run", fake_run)

    live_test_session.build_wup_runtime(tmp_path, {"PATH": "base-path"})

    assert calls == [
        (
            [
                "cargo.exe",
                "build",
                "-q",
                "-p",
                "mole_runtime",
                "--features",
                "wup",
                "--target-dir",
                str(tmp_path / "target-wup"),
            ],
            {"cwd": tmp_path, "env": {"PATH": "base-path"}},
        )
    ]


def test_latest_log_returns_newest_matching_file(tmp_path):
    old_log = tmp_path / "input-debug-20260527-100000.jsonl"
    new_log = tmp_path / "input-debug-20260527-100001.jsonl"
    old_log.write_text("old", encoding="utf-8")
    new_log.write_text("new", encoding="utf-8")
    os.utime(old_log, (1, 1))
    os.utime(new_log, (2, 2))

    assert live_test_session.latest_log(tmp_path, "input-debug-") == new_log


def test_instance_lock_rejects_second_holder(tmp_path):
    lock_path = tmp_path / "logs" / "live-test-session.lock"
    first_lock = live_test_session.acquire_instance_lock(lock_path)
    assert first_lock is not None

    try:
        assert live_test_session.acquire_instance_lock(lock_path) is None
    finally:
        first_lock.release()

    second_lock = live_test_session.acquire_instance_lock(lock_path)
    assert second_lock is not None
    second_lock.release()


def test_run_live_session_refuses_second_active_session(monkeypatch, tmp_path):
    lock_path = live_test_session.live_session_lock_path(tmp_path)
    first_lock = live_test_session.acquire_instance_lock(lock_path)
    assert first_lock is not None

    started = False

    def start_game(*args):
        nonlocal started
        started = True
        raise AssertionError("second live session should not start a game")

    monkeypatch.setattr(live_test_session, "start_game", start_game)

    try:
        result = live_test_session.run_live_session(
            tmp_path,
            python=Path("python.exe"),
            poll_interval=0,
            build_rust=False,
        )
    finally:
        first_lock.release()

    assert result == live_test_session.ALREADY_RUNNING_EXIT_CODE
    assert started is False


def test_run_live_session_refuses_nested_child_environment(monkeypatch, tmp_path):
    started = False

    def start_game(*args):
        nonlocal started
        started = True
        raise AssertionError("nested live session should not start a game")

    monkeypatch.setenv("MOLE_LIVE_SESSION_CHILD", "1")
    monkeypatch.setattr(live_test_session, "start_game", start_game)

    result = live_test_session.run_live_session(
        tmp_path,
        python=Path("python.exe"),
        poll_interval=0,
        build_rust=False,
    )

    assert result == live_test_session.ALREADY_RUNNING_EXIT_CODE
    assert started is False


def test_stop_game_terminates_process_tree_on_windows(monkeypatch):
    class FakeProcess:
        pid = 1234

        def __init__(self):
            self.terminated = False
            self.killed = False
            self.wait_timeouts = []

        def poll(self):
            return None

        def terminate(self):
            self.terminated = True

        def kill(self):
            self.killed = True

        def wait(self, timeout=None):
            self.wait_timeouts.append(timeout)

    calls = []

    def fake_run(command, **kwargs):
        calls.append(command)

        class Result:
            returncode = 0

        return Result()

    monkeypatch.setattr(live_test_session.os, "name", "nt")
    monkeypatch.setattr(live_test_session.subprocess, "run", fake_run)

    process = FakeProcess()
    live_test_session.stop_game(process)

    assert calls == [["taskkill", "/PID", "1234", "/T", "/F"]]
    assert process.terminated is False
    assert process.killed is False
    assert process.wait_timeouts == [3.0]


def test_run_live_session_exits_when_game_crashes(monkeypatch, tmp_path):
    class FakeProcess:
        def poll(self):
            return 1

    reported = []
    monkeypatch.setattr(live_test_session, "start_game", lambda *args: FakeProcess())
    monkeypatch.setattr(live_test_session, "collect_snapshot", lambda root: {})
    monkeypatch.setattr(live_test_session, "report_latest_logs", lambda root: reported.append(root))

    result = live_test_session.run_live_session(
        tmp_path,
        python=Path("python.exe"),
        poll_interval=0,
        build_rust=False,
    )

    assert result == 1
    assert reported == [tmp_path.resolve()]


def test_start_state_graph_viewer_launches_viewer_child(monkeypatch, tmp_path):
    viewer = tmp_path / "tools" / "state_graph_viewer.py"
    viewer.parent.mkdir()
    viewer.write_text("print('viewer')", encoding="utf-8")
    calls = []

    class FakeProcess:
        pass

    def fake_popen(command, **kwargs):
        calls.append((command, kwargs))
        return FakeProcess()

    monkeypatch.setattr(live_test_session.subprocess, "Popen", fake_popen)

    process = live_test_session.start_state_graph_viewer(
        tmp_path,
        Path("python.exe"),
        {"ENV": "1"},
    )

    assert isinstance(process, FakeProcess)
    assert calls == [
        (
            ["python.exe", str(viewer)],
            {"cwd": tmp_path.resolve(), "env": {"ENV": "1"}},
        )
    ]


def test_run_live_session_closes_state_graph_viewer_with_game(monkeypatch, tmp_path):
    class GameProcess:
        def poll(self):
            return 0

    class ViewerProcess:
        def poll(self):
            return None

    viewer = ViewerProcess()
    stopped = []
    monkeypatch.setattr(live_test_session, "start_game", lambda *args: GameProcess())
    monkeypatch.setattr(live_test_session, "start_state_graph_viewer", lambda *args: viewer)
    monkeypatch.setattr(live_test_session, "stop_game", lambda process: stopped.append(process))
    monkeypatch.setattr(live_test_session, "collect_snapshot", lambda root: {})

    result = live_test_session.run_live_session(
        tmp_path,
        python=Path("python.exe"),
        poll_interval=0,
        build_rust=False,
    )

    assert result == 0
    assert stopped == [viewer]


def test_run_live_session_does_not_restart_on_changes_by_default(monkeypatch, tmp_path):
    class FakeProcess:
        def __init__(self):
            self.polls = 0

        def poll(self):
            self.polls += 1
            return 0 if self.polls >= 5 else None

    snapshots = [
        {"ChooseAction.py": (1, 1)},
        {"ChooseAction.py": (2, 1)},
        {"ChooseAction.py": (2, 1)},
        {"ChooseAction.py": (2, 1)},
        {"ChooseAction.py": (2, 1)},
    ]
    monotonic_values = iter([0.0, 1.0, 2.0])
    started = []
    stopped = []

    def collect_snapshot(_root):
        if len(snapshots) > 1:
            return snapshots.pop(0)
        return snapshots[0]

    monkeypatch.setattr(live_test_session, "collect_snapshot", collect_snapshot)
    monkeypatch.setattr(live_test_session, "start_game", lambda *args: started.append(args) or FakeProcess())
    monkeypatch.setattr(live_test_session, "stop_game", lambda process: stopped.append(process))
    monkeypatch.setattr(live_test_session.time, "sleep", lambda _seconds: None)
    monkeypatch.setattr(live_test_session.time, "monotonic", lambda: next(monotonic_values))

    result = live_test_session.run_live_session(
        tmp_path,
        python=Path("python.exe"),
        poll_interval=0,
        build_rust=False,
    )

    assert result == 0
    assert len(started) == 1
    assert stopped == []


def test_parse_args_keeps_auto_restart_opt_in():
    args = live_test_session.parse_args([])
    assert args.auto_restart is False
    assert args.state_graph is True

    args = live_test_session.parse_args(["--auto-restart"])
    assert args.auto_restart is True

    args = live_test_session.parse_args(["--no-state-graph"])
    assert args.state_graph is False
