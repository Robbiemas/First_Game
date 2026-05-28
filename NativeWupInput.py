import json
import os
from datetime import datetime
from pathlib import Path
import shutil
import subprocess
import threading


PROJECT_ROOT = Path(__file__).resolve().parent
PLAYER_COUNT = 2
GAMECUBE_STICK_CENTER = 128
GAMECUBE_AXIS_NAMES = ("main_x", "main_y", "c_x", "c_y")
GAMECUBE_TRIGGER_NAMES = ("left_trigger", "right_trigger")
GAMECUBE_ORIGIN_NAMES = GAMECUBE_AXIS_NAMES + GAMECUBE_TRIGGER_NAMES
GAMECUBE_RECENTER_FRAMES = 180


def _runtime_exe_name():
    return "mole_runtime.exe" if os.name == "nt" else "mole_runtime"


class GameCubeCalibration:
    def __init__(self, axes=None):
        self.axes = {axis: {"center": GAMECUBE_STICK_CENTER} for axis in GAMECUBE_AXIS_NAMES}

    def normalize_raw_axis(self, axis, value, origin=None):
        center = GAMECUBE_STICK_CENTER if origin is None else int(origin)
        centered = int(value) - center
        if centered < 0:
            return round(max(centered / 128, -1.0), 3)

        return round(min(centered / 127, 1.0), 3)

    def axis_label(self, axis):
        return f"{axis}=native-origin"


def load_gamecube_calibration(project_root=PROJECT_ROOT):
    return GameCubeCalibration()


def _empty_state():
    return {
        "connected": False,
        "source_port": None,
        "raw_main_x": None,
        "raw_main_y": None,
        "raw_c_x": None,
        "raw_c_y": None,
        "main_x": 0,
        "main_y": 0,
        "c_x": 0,
        "c_y": 0,
        "left_trigger": 0,
        "right_trigger": 0,
        "a": False,
        "b": False,
        "x": False,
        "y": False,
        "z": False,
        "l": False,
        "r": False,
        "start": False,
        "dpad_up": False,
        "dpad_down": False,
        "dpad_left": False,
        "dpad_right": False,
        "melee": None,
    }


class NativeWupPad:
    def __init__(self, player_index, calibration=None):
        self.player_index = player_index
        self.source_port = None
        self.calibration = calibration or GameCubeCalibration()
        self._state = _empty_state()
        self.origin = {name: None for name in GAMECUBE_ORIGIN_NAMES}
        self._recenter_frames = 0

    def apply_state(self, state):
        next_state = _empty_state()
        next_state.update(state)
        self.source_port = next_state["source_port"]
        self._state = next_state
        if not self.connected():
            self._clear_origin()
            return

        if self._has_raw_gamecube_state():
            if not self._has_origin():
                self._capture_origin()
            self._update_recenter_combo()

    def connected(self):
        return bool(self._state["connected"])

    def get_name(self):
        if self.source_port is None:
            return f"Native WUP-028 Player {self.player_index + 1}"
        return f"Native WUP-028 Port {self.source_port + 1}"

    def get_numaxes(self):
        return 6

    def get_axis(self, index):
        melee = self._melee_state()
        axes = (
            _melee_stick_axis(melee, "lstick_x")
            if _melee_has(melee, "lstick_x")
            else _state_stick_axis(
                self._state,
                "raw_main_x",
                "main_x",
                self.calibration,
                "main_x",
                self.origin["main_x"],
            ),
            -_melee_stick_axis(melee, "lstick_y")
            if _melee_has(melee, "lstick_y")
            else -_state_stick_axis(
                self._state,
                "raw_main_y",
                "main_y",
                self.calibration,
                "main_y",
                self.origin["main_y"],
            ),
            _melee_stick_axis(melee, "cstick_x")
            if _melee_has(melee, "cstick_x")
            else _state_stick_axis(
                self._state,
                "raw_c_x",
                "c_x",
                self.calibration,
                "c_x",
                self.origin["c_x"],
            ),
            -_melee_stick_axis(melee, "cstick_y")
            if _melee_has(melee, "cstick_y")
            else -_state_stick_axis(
                self._state,
                "raw_c_y",
                "c_y",
                self.calibration,
                "c_y",
                self.origin["c_y"],
            ),
            _trigger_byte_axis(melee["left_trigger"])
            if _melee_has(melee, "left_trigger")
            else _trigger_axis(self._state["left_trigger"], self.origin["left_trigger"]),
            _trigger_byte_axis(melee["right_trigger"])
            if _melee_has(melee, "right_trigger")
            else _trigger_axis(self._state["right_trigger"], self.origin["right_trigger"]),
        )
        if index >= len(axes):
            return 0
        return axes[index]

    def get_numbuttons(self):
        return 12

    def get_button(self, index):
        buttons = (
            "a",
            "b",
            "x",
            "y",
            "z",
            "l",
            "r",
            "start",
            "dpad_up",
            "dpad_down",
            "dpad_left",
            "dpad_right",
        )
        if index >= len(buttons):
            return False
        return bool(self._state[buttons[index]])

    def get_melee_fact(self, name, default=None):
        melee = self._melee_state()
        if melee is None:
            return default
        return melee.get(name, default)

    def debug_lines(self):
        return [
            (
                "raw: "
                f"main=({_debug_raw(self._state['raw_main_x'])},{_debug_raw(self._state['raw_main_y'])}) "
                f"c=({_debug_raw(self._state['raw_c_x'])},{_debug_raw(self._state['raw_c_y'])})"
            ),
            (
                "origin: "
                f"main=({_debug_raw(self.origin['main_x'])},{_debug_raw(self.origin['main_y'])}) "
                f"c=({_debug_raw(self.origin['c_x'])},{_debug_raw(self.origin['c_y'])}) "
                f"triggers=({_debug_raw(self.origin['left_trigger'])},{_debug_raw(self.origin['right_trigger'])})"
            ),
            "native: console origin, no endpoint calibration",
        ]

    def _has_raw_gamecube_state(self):
        return all(self._state.get(f"raw_{axis}") is not None for axis in GAMECUBE_AXIS_NAMES)

    def _has_origin(self):
        return all(self.origin[name] is not None for name in GAMECUBE_ORIGIN_NAMES)

    def _capture_origin(self):
        for axis in GAMECUBE_AXIS_NAMES:
            self.origin[axis] = int(self._state[f"raw_{axis}"])
        for trigger in GAMECUBE_TRIGGER_NAMES:
            self.origin[trigger] = int(self._state[trigger])

    def _clear_origin(self):
        for name in GAMECUBE_ORIGIN_NAMES:
            self.origin[name] = None
        self._recenter_frames = 0

    def _update_recenter_combo(self):
        if self._state["x"] and self._state["y"] and self._state["start"]:
            self._recenter_frames += 1
            if self._recenter_frames >= GAMECUBE_RECENTER_FRAMES:
                self._capture_origin()
                self._recenter_frames = 0
            return

        self._recenter_frames = 0

    def _melee_state(self):
        melee = self._state.get("melee")
        return melee if isinstance(melee, dict) else None


class NativeWupAdapter:
    def __init__(self, project_root=PROJECT_ROOT):
        self.project_root = Path(project_root)
        self.calibration = load_gamecube_calibration(self.project_root)
        self.pads = [NativeWupPad(index, self.calibration) for index in range(PLAYER_COUNT)]
        self._process = None
        self._reader = None
        self._command = None
        self._stopping = False
        self._stderr_handle = None
        self._stderr_log_path = None

    def start(self):
        if os.environ.get("MOLE_DISABLE_NATIVE_WUP"):
            return False

        command = self._stream_command()
        if command is None:
            return False

        self._command = command
        self._stopping = False
        self._reader = threading.Thread(target=self._stream_loop, daemon=True)
        self._reader.start()
        return True

    def input_sources(self):
        return [pad if pad.connected() else None for pad in self.pads]

    def stop(self):
        self._stopping = True
        process = self._process
        reader = self._reader
        if process is not None and process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=1.0)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=1.0)
        if reader is not None and reader is not threading.current_thread():
            reader.join(timeout=1.0)
        self._process = None
        self._reader = None
        self._clear_pads()

    def _stream_command(self):
        configured_runtime = os.environ.get("MOLE_RUNTIME_EXE")
        if configured_runtime:
            runtime_exe = Path(configured_runtime)
            if runtime_exe.exists():
                return [str(runtime_exe), "--stream-wup"]

        runtime_exe = self.project_root / "target-wup" / "debug" / _runtime_exe_name()
        if runtime_exe.exists():
            return [str(runtime_exe), "--stream-wup"]

        cargo = shutil.which("cargo")
        if cargo is None:
            cargo = Path.home() / ".cargo" / "bin" / "cargo.exe"
            if not cargo.exists():
                return None
            cargo = str(cargo)

        return [
            cargo,
            "run",
            "-q",
            "-p",
            "mole_runtime",
            "--features",
            "wup",
            "--",
            "--stream-wup",
        ]

    def _open_stream_process(self):
        if self._command is None:
            return None

        flags = getattr(subprocess, "CREATE_NO_WINDOW", 0)
        self._open_helper_log()
        try:
            return subprocess.Popen(
                self._command,
                cwd=self.project_root,
                stdout=subprocess.PIPE,
                stderr=self._stderr_handle or subprocess.DEVNULL,
                text=True,
                bufsize=1,
                creationflags=flags,
            )
        except OSError:
            self._write_helper_log("Failed to start WUP helper process.")
            self._close_helper_log()
            return None

    def _stream_loop(self):
        if self._command is None:
            self._command = self._stream_command()
        if self._command is None:
            self._clear_pads()
            return

        process = self._open_stream_process()
        if process is None:
            self._clear_pads()
            return

        self._process = process
        self._read_stream_process(process)
        self._finish_stream_process(process)
        self._clear_pads()

    def _read_loop(self):
        if self._process is None:
            self._clear_pads()
            return
        self._read_stream_process(self._process)
        self._clear_pads()

    def _read_stream_process(self, process):
        if process is None or process.stdout is None:
            return

        for line in process.stdout:
            if self._stopping:
                break
            apply_stream_line(self.pads, line)

    def _finish_stream_process(self, process):
        if process is None:
            return
        if process.poll() is None:
            self._write_helper_log("Terminating WUP helper after stream loop ended.")
            process.terminate()
        try:
            exit_code = process.wait(timeout=1.0)
        except subprocess.TimeoutExpired:
            self._write_helper_log("Killing unresponsive WUP helper process.")
            process.kill()
            exit_code = process.wait(timeout=1.0)
        finally:
            if "exit_code" not in locals():
                exit_code = process.poll()
            self._write_helper_log(f"WUP helper exited with code {exit_code}.")
            self._close_helper_log()
            if self._process is process:
                self._process = None

    def _clear_pads(self):
        for pad in self.pads:
            pad.apply_state(_empty_state())

    def _open_helper_log(self):
        if self._stderr_handle is not None:
            return
        log_dir = self.project_root / "logs"
        log_dir.mkdir(parents=True, exist_ok=True)
        stamp = datetime.now().strftime("%Y%m%d-%H%M%S-%f")
        self._stderr_log_path = log_dir / f"wup-helper-{stamp}.log"
        self._stderr_handle = self._stderr_log_path.open("w", encoding="utf-8")
        self._write_helper_log(f"Starting WUP helper: {' '.join(self._command or [])}")

    def _write_helper_log(self, message):
        if self._stderr_handle is None:
            return
        self._stderr_handle.write(f"[mole] {message}\n")
        self._stderr_handle.flush()

    def _close_helper_log(self):
        if self._stderr_handle is None:
            return
        self._stderr_handle.close()
        self._stderr_handle = None


def create_native_wup_adapter():
    adapter = NativeWupAdapter()
    if adapter.start():
        return adapter
    return None


def apply_stream_line(pads, line):
    try:
        payload = json.loads(line)
    except json.JSONDecodeError:
        return

    players = payload.get("players", [])
    for index, pad in enumerate(pads):
        if index < len(players):
            pad.apply_state(players[index])
        else:
            pad.apply_state(_empty_state())


def _stick_axis(value):
    value = int(value)
    if value < 0:
        return round(max(value / 32768, -1.0), 3)
    return round(min(value / 32767, 1.0), 3)


def _melee_has(melee, key):
    return melee is not None and key in melee


def _melee_stick_axis(melee, key):
    value = int(melee[key])
    if value < 0:
        return round(max(value / 128, -1.0), 3)
    return round(min(value / 127, 1.0), 3)


def _state_stick_axis(state, raw_key, centered_key, calibration, axis, origin=None):
    raw_value = state.get(raw_key)
    if raw_value is not None:
        return calibration.normalize_raw_axis(axis, raw_value, origin)
    return _stick_axis(state[centered_key])


def _raw_stick_axis(value):
    value = int(value)
    return GameCubeCalibration().normalize_raw_axis("main_x", value)


def _trigger_axis(value, origin=None):
    value = int(value)
    origin = 0 if origin is None else int(origin)
    return max(0.0, min((value - origin) / 255, 1.0))


def _trigger_byte_axis(value):
    return max(0.0, min(int(value) / 255, 1.0))


def _debug_raw(value):
    return "?" if value is None else str(int(value))
