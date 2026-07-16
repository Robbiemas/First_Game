import importlib
import sys
from pathlib import Path

import pygame

ROOT = Path(__file__).resolve().parents[1]


class PlayerStub:
    pass


class PressedKeys:
    def __init__(self, pressed):
        self.pressed = set(pressed)

    def __getitem__(self, key):
        return key in self.pressed


def fresh_main_module(monkeypatch):
    monkeypatch.setenv("SDL_VIDEODRIVER", "dummy")
    sys.modules.pop("RealMainFile", None)
    return importlib.import_module("RealMainFile")


def test_importing_main_module_does_not_start_game_or_require_joysticks(monkeypatch):
    module = fresh_main_module(monkeypatch)

    assert callable(module.gameloop)
    assert hasattr(module, "init_joysticks")


def test_init_joysticks_handles_no_connected_controllers(monkeypatch):
    module = fresh_main_module(monkeypatch)

    class FakeJoystick:
        def init(self):
            pass

        def get_count(self):
            return 0

        def Joystick(self, index):
            raise AssertionError("Joystick should not be created when count is zero")

    fake_pygame = type("FakePygame", (), {"joystick": FakeJoystick()})

    assert module.init_joysticks(fake_pygame) == []


def test_keyboard_is_enabled_for_player_one_even_when_controller_is_connected(monkeypatch):
    module = fresh_main_module(monkeypatch)

    _player1_source, _player2_source, keyboard_player1 = module.player_input_sources([object()])

    assert keyboard_player1 is True


def test_main_module_enables_sdl_gamecube_hidapi(monkeypatch):
    module = fresh_main_module(monkeypatch)

    module.configure_sdl_controller_hints()

    assert module.os.environ["SDL_JOYSTICK_HIDAPI"] == "1"
    assert module.os.environ["SDL_JOYSTICK_HIDAPI_GAMECUBE"] == "1"


def test_sdl3_runtime_launcher_builds_with_native_wup_feature():
    launcher = ROOT / "execs" / "Run SDL3 Runtime.cmd"

    text = launcher.read_text(encoding="utf-8")

    assert 'set "RUNTIME_EXE=%CD%\\target\\release\\mole_runtime.exe"' in text
    assert 'if not exist "%SDL3_ROOT%\\lib\\x64\\SDL3.dll"' in text
    assert "Ensure Release Runtime.cmd" in text
    assert '"%RUNTIME_EXE%" --sdl --play --input-trace' in text
    assert "-- --sdl" not in text
    assert "--input-trace" in text
    assert "cargo run --release" not in text
    assert "--no-ucf" not in text
    assert "--features sdl -- --sdl" not in text


def test_sdl3_runtime_vanilla_launcher_disables_ucf():
    launcher = ROOT / "execs" / "Run SDL3 Runtime Vanilla No UCF.cmd"

    text = launcher.read_text(encoding="utf-8")

    assert 'set "RUNTIME_EXE=%CD%\\target\\release\\mole_runtime.exe"' in text
    assert 'if not exist "%SDL3_ROOT%\\lib\\x64\\SDL3.dll"' in text
    assert "Ensure Release Runtime.cmd" in text
    assert '"%RUNTIME_EXE%" --sdl --play --input-trace --no-ucf' in text
    assert "cargo run --release" not in text
    assert "Open Dev Tool.cmd" in text


def test_sdl3_runtime_launcher_opens_state_graph_viewer():
    launcher = ROOT / "execs" / "Run SDL3 Runtime.cmd"

    text = launcher.read_text(encoding="utf-8")

    assert "Open Dev Tool.cmd" in text
    assert 'start "Mole Game Dev Tool"' in text
    assert "tools\\state_graph_viewer.py" not in text


def test_start_here_launcher_exposes_replay_local_and_devtool_entrypoints():
    launcher = ROOT / "START HERE - Mole Game.hta"

    text = launcher.read_text(encoding="utf-8")

    assert "<HTA:APPLICATION" in text
    assert "WScript.Shell" in text
    assert "runCommandFileHidden" in text
    assert "startActiveStatus" in text
    assert "progressBar" in text
    assert "runtime-launch.latest.log" in text
    assert "progressForLogLine" in text
    assert "copyStatusText" in text
    assert 'id="copyStatusButton"' in text
    assert 'onclick="copyStatusText()"' in text
    assert 'window.clipboardData.setData("Text", text)' in text
    assert "Launching Mole Rust SDL3 Slippi replay runtime" in text
    assert "statusDirection" not in text
    assert "window.resizeTo(600, 580)" in text
    assert "overflow: auto" in text
    assert text.count("<button") == 4
    assert "execs\\\\Play Slippi Replay.cmd" in text
    assert "execs\\\\Run Local SDL3 Runtime.cmd" in text
    assert "execs\\\\Open Dev Tool.cmd" in text
    assert "debug\\\\slippi\\\\runtime-divergence.latest.json" in text
    assert "Writes the live divergence log and holds on the first mismatch." in text
    assert "Uses replays\\Game_20260530T214929.slp and writes debug\\slippi\\runtime-divergence.latest.json." not in text
    assert "playtest\\\\MoleGame-FriendPlaytest.exe" not in text
    assert "playtest\\\\MoleGame-LocalInternetPlaytest.exe" not in text


def test_slippi_replay_launcher_uses_replay_source_and_live_divergence_log():
    launcher = ROOT / "execs" / "Play Slippi Replay.cmd"

    text = launcher.read_text(encoding="utf-8")

    assert 'set "REPLAY_PATH=%CD%\\replays\\Game_20260530T214929.slp"' in text
    assert 'set "DIVERGENCE_LOG=%CD%\\debug\\slippi\\runtime-divergence.latest.json"' in text
    assert 'set "RUNTIME_EXE=%CD%\\target\\release\\mole_runtime.exe"' in text
    assert 'if not exist "%SDL3_ROOT%\\lib\\x64\\SDL3.dll"' in text
    assert "Ensure Release Runtime.cmd" in text
    assert '"%RUNTIME_EXE%" --sdl' in text
    assert '--visual-slippi-replay "%REPLAY_PATH%"' in text
    assert '--frames 4294967295' in text
    assert '--slippi-divergence-log "%DIVERGENCE_LOG%"' in text
    assert "--hold-final-frame" in text
    assert "cargo run --release" not in text
    assert "slippi_replay_to_inputs.cjs" not in text
    assert ".inputs.json" not in text
    assert "Open Dev Tool.cmd" not in text


def test_release_runtime_helper_rebuilds_when_runtime_sources_are_newer():
    launcher = ROOT / "execs" / "Ensure Release Runtime.cmd"

    text = launcher.read_text(encoding="utf-8")

    assert 'set "RUNTIME_EXE=%CD%\\target\\release\\mole_runtime.exe"' in text
    assert 'if not exist "%RUNTIME_EXE%"' in text
    assert "LastWriteTimeUtc" in text
    assert "crates\\mole_core\\src" in text
    assert "crates\\mole_runtime\\src" in text
    assert "Runtime source newer than release exe" in text
    assert 'cargo build --release -p mole_runtime --features "sdl wup"' in text


def test_clean_local_outputs_launcher_is_dry_run_and_avoids_required_artifacts():
    launcher = ROOT / "execs" / "Clean Local Outputs.cmd"

    text = launcher.read_text(encoding="utf-8")

    assert "Dry run" in text
    assert "--apply" in text
    assert "Remove-Item" in text
    assert "GetFullPath" in text
    assert "StartsWith($rootFull" in text
    assert "'debug'" in text
    assert "'logs'" in text
    assert "'.pytest_cache'" in text
    assert "'__pycache__'" in text
    assert "'target'" not in text
    assert "'.local'" not in text
    assert "'.venv'" not in text
    assert "'replays'" not in text
    assert "crates\\mole_runtime\\src\\generated" not in text


def test_deprecated_execs_are_out_of_the_active_launcher_folder():
    active_execs = ROOT / "execs"
    deprecated_execs = active_execs / "depreciated"
    deprecated_names = {
        "Check Controllers.cmd",
        "Check SDL3 Inputs.cmd",
        "Launch Mole Game.cmd",
        "Launch Mole Game Debug.cmd",
        "Live Test Session.cmd",
        "Open GameCube Calibration.cmd",
        "Run WUP Native Runtime.cmd",
    }

    for name in deprecated_names:
        assert not (active_execs / name).exists()
        assert (deprecated_execs / name).is_file()


def test_main_menu_is_skipped_by_default(monkeypatch):
    module = fresh_main_module(monkeypatch)

    monkeypatch.delenv("MOLE_SHOW_MENU", raising=False)

    assert module.should_show_main_menu(max_frames=None) is False


def test_main_menu_can_be_enabled_with_environment_flag(monkeypatch):
    module = fresh_main_module(monkeypatch)

    monkeypatch.setenv("MOLE_SHOW_MENU", "1")

    assert module.should_show_main_menu(max_frames=None) is True


def test_main_menu_stays_skipped_for_bounded_smoke_runs(monkeypatch):
    module = fresh_main_module(monkeypatch)

    monkeypatch.setenv("MOLE_SHOW_MENU", "1")

    assert module.should_show_main_menu(max_frames=1) is False


def test_gather_inputs_can_idle_without_a_joystick():
    from GatherInputs import gather_inputs

    player = PlayerStub()

    gather_inputs(player, None)

    assert player.main_stick == [0, 0]
    assert player.c_stick == [0, 0]
    assert player.jumpkey is False
    assert player.canJump is True
    assert player.blockkey is False
    assert player.canBlock is True
    assert player.menukey is False
    assert player.menu is True


def test_gather_inputs_maps_keyboard_for_player_one():
    from GatherInputs import gather_inputs

    player = PlayerStub()
    keys = PressedKeys(
        {
            pygame.K_d,
            pygame.K_w,
            pygame.K_j,
            pygame.K_k,
            pygame.K_l,
            pygame.K_p,
        }
    )

    gather_inputs(player, None, keys=keys, keyboard=True)

    assert player.main_stick == [1, -1]
    assert player.akey is True
    assert player.jumpkey is True
    assert player.blockkey is True
    assert player.menukey is True


def test_remembered_keyboard_controls_use_w_or_up_to_jump_and_space_to_attack():
    from GatherInputs import gather_inputs

    player = PlayerStub()
    keys = PressedKeys({pygame.K_w, pygame.K_SPACE})

    gather_inputs(player, None, keys=keys, keyboard=True)

    assert player.main_stick == [0, -1]
    assert player.jumpkey is True
    assert player.akey is True


class FakeJoystick:
    def __init__(self, name="Generic Controller", axes=None, buttons=None):
        self.name = name
        self.axes = axes or {}
        self.buttons = buttons or {}

    def get_name(self):
        return self.name

    def get_numaxes(self):
        return 6

    def get_axis(self, index):
        return self.axes.get(index, 0)

    def get_numbuttons(self):
        return 12

    def get_button(self, index):
        return self.buttons.get(index, False)


def trigger_after_deadzone(value):
    return 0 if value <= 0.2 else (value - 0.2) / 0.8


def test_keyboard_still_controls_player_one_when_controller_is_connected():
    from GatherInputs import gather_inputs

    player = PlayerStub()
    joystick = FakeJoystick()
    keys = PressedKeys({pygame.K_d, pygame.K_w})

    gather_inputs(player, joystick, keys=keys, keyboard=True)

    assert player.main_stick == [1, -1]
    assert player.jumpkey is True


def test_xbox_trigger_axes_map_to_block():
    from GatherInputs import gather_inputs

    player = PlayerStub()
    joystick = FakeJoystick("Xbox 360 Controller", axes={2: 1.0})

    gather_inputs(player, joystick)

    assert player.blockkey is True


def test_trigger_values_below_deadzone_do_not_create_shield_input():
    from GatherInputs import gather_inputs

    player = PlayerStub()
    joystick = FakeJoystick("Native WUP-028 Port 1", axes={4: 0.19, 5: 0.0})

    gather_inputs(player, joystick)

    assert player.l_trigger == 0
    assert player.l_shieldkey is False
    assert player.blockkey is False


def test_trigger_value_at_deadzone_still_reads_as_zero():
    from GatherInputs import gather_inputs

    player = PlayerStub()
    joystick = FakeJoystick("Native WUP-028 Port 1", axes={4: 0.2, 5: 0.0})

    gather_inputs(player, joystick)

    assert player.l_trigger == 0
    assert player.l_shieldkey is False
    assert player.blockkey is False


def test_trigger_values_above_deadzone_rescale_from_zero():
    from GatherInputs import gather_inputs

    player = PlayerStub()
    joystick = FakeJoystick("Native WUP-028 Port 1", axes={4: 0.36, 5: 0.0})

    gather_inputs(player, joystick)

    assert player.l_trigger == trigger_after_deadzone(0.36)
    assert player.l_shieldkey is True
    assert player.blockkey is True


def test_native_wup_pad_maps_full_gamecube_controls_to_legacy_game_input():
    from GatherInputs import gather_inputs
    from NativeWupInput import NativeWupPad

    player = PlayerStub()
    pad = NativeWupPad(0)
    pad.apply_state(
        {
            "connected": True,
            "source_port": 1,
            "main_x": 32767,
            "main_y": -32768,
            "c_x": -32768,
            "c_y": 32512,
            "left_trigger": 128,
            "right_trigger": 255,
            "a": True,
            "b": True,
            "x": False,
            "y": True,
            "z": True,
            "l": True,
            "r": False,
            "start": True,
            "dpad_up": True,
            "dpad_down": False,
            "dpad_left": False,
            "dpad_right": True,
        }
    )

    gather_inputs(player, pad)

    assert player.main_stick == [1.0, 1.0]
    assert player.c_stick == [-1.0, -0.992]
    assert player.l_trigger == trigger_after_deadzone(128 / 255)
    assert player.r_trigger == 1.0
    assert player.l_trigger_digital is True
    assert player.r_trigger_digital is False
    assert player.akey is True
    assert player.specialkey is True
    assert player.jumpkey is True
    assert player.grabkey is True
    assert player.blockkey is True
    assert player.menukey is True
    assert player.upkey is True
    assert player.rightkey is True


def test_native_wup_pad_accepts_raw_gamecube_stream_values():
    from GatherInputs import gather_inputs
    from NativeWupInput import NativeWupPad

    player = PlayerStub()
    pad = NativeWupPad(0)
    pad.apply_state(
        {
            "connected": True,
            "source_port": 0,
            "raw_main_x": 128,
            "raw_main_y": 128,
            "raw_c_x": 128,
            "raw_c_y": 128,
            "left_trigger": 0,
            "right_trigger": 0,
        }
    )
    pad.apply_state(
        {
            "connected": True,
            "source_port": 0,
            "raw_main_x": 255,
            "raw_main_y": 0,
            "raw_c_x": 0,
            "raw_c_y": 255,
            "left_trigger": 64,
            "right_trigger": 255,
            "a": True,
            "b": False,
            "x": True,
            "y": False,
            "z": False,
            "l": False,
            "r": True,
            "start": False,
            "dpad_up": False,
            "dpad_down": True,
            "dpad_left": False,
            "dpad_right": False,
        }
    )

    gather_inputs(player, pad)

    assert player.main_stick == [1.0, 1.0]
    assert player.c_stick == [-1.0, -1.0]
    assert player.l_trigger == trigger_after_deadzone(64 / 255)
    assert player.r_trigger == 1.0
    assert player.l_trigger_digital is False
    assert player.r_trigger_digital is True
    assert player.akey is True
    assert player.jumpkey is True
    assert player.blockkey is True
    assert player.downkey is True


def test_gamecube_shield_inputs_track_analog_digital_and_per_side_edges():
    from GatherInputs import gather_inputs
    from NativeWupInput import NativeWupPad

    player = PlayerStub()
    pad = NativeWupPad(0)
    pad.apply_state(
        {
            "connected": True,
            "raw_main_x": 128,
            "raw_main_y": 128,
            "raw_c_x": 128,
            "raw_c_y": 128,
            "left_trigger": 0,
            "right_trigger": 0,
        }
    )
    pad.apply_state(
        {
            "connected": True,
            "raw_main_x": 128,
            "raw_main_y": 128,
            "raw_c_x": 128,
            "raw_c_y": 128,
            "left_trigger": 180,
            "right_trigger": 0,
            "l": False,
            "r": False,
        }
    )
    gather_inputs(player, pad)

    assert player.l_trigger == trigger_after_deadzone(180 / 255)
    assert player.l_trigger_digital is False
    assert player.l_trigger_digital_pressed is False
    assert player.l_shieldkey is True
    assert player.l_shield_pressed is True
    assert player.r_shieldkey is False

    pad.apply_state(
        {
            "connected": True,
            "raw_main_x": 128,
            "raw_main_y": 128,
            "raw_c_x": 128,
            "raw_c_y": 128,
            "left_trigger": 180,
            "right_trigger": 255,
            "l": False,
            "r": True,
        }
    )
    gather_inputs(player, pad)

    assert player.r_trigger == 1.0
    assert player.r_trigger_digital is True
    assert player.r_trigger_digital_pressed is True
    assert player.l_shieldkey is True
    assert player.l_shield_pressed is False
    assert player.r_shieldkey is True
    assert player.r_shield_pressed is True


def test_native_wup_pad_preserves_native_gate_distance_without_endpoint_stretching():
    from NativeWupInput import NativeWupPad

    pad = NativeWupPad(0)
    pad.apply_state(
        {
            "connected": True,
            "raw_main_x": 128,
            "raw_main_y": 128,
            "raw_c_x": 128,
            "raw_c_y": 128,
        }
    )
    pad.apply_state(
        {
            "connected": True,
            "raw_main_x": 232,
            "raw_main_y": 128,
            "raw_c_x": 20,
            "raw_c_y": 128,
        }
    )

    assert pad.get_axis(0) == 0.819
    assert pad.get_axis(1) == 0.0
    assert pad.get_axis(2) == -0.844
    assert pad.get_axis(3) == 0.0


def test_native_wup_pad_auto_captures_first_raw_origin_for_plug_and_play_neutral():
    from NativeWupInput import NativeWupPad

    pad = NativeWupPad(0)
    pad.apply_state(
        {
            "connected": True,
            "raw_main_x": 131,
            "raw_main_y": 125,
            "raw_c_x": 127,
            "raw_c_y": 130,
            "left_trigger": 7,
            "right_trigger": 4,
        }
    )

    assert pad.get_axis(0) == 0.0
    assert pad.get_axis(1) == 0.0
    assert pad.get_axis(2) == 0.0
    assert pad.get_axis(3) == 0.0
    assert pad.get_axis(4) == 0.0
    assert pad.get_axis(5) == 0.0


def test_native_wup_pad_offsets_raw_axes_from_auto_captured_origin_without_rescaling():
    from NativeWupInput import NativeWupPad

    pad = NativeWupPad(0)
    pad.apply_state(
        {
            "connected": True,
            "raw_main_x": 131,
            "raw_main_y": 125,
            "raw_c_x": 127,
            "raw_c_y": 130,
            "left_trigger": 7,
            "right_trigger": 4,
        }
    )
    pad.apply_state(
        {
            "connected": True,
            "raw_main_x": 232,
            "raw_main_y": 20,
            "raw_c_x": 20,
            "raw_c_y": 232,
            "left_trigger": 131,
            "right_trigger": 255,
        }
    )

    assert pad.get_axis(0) == 0.795
    assert pad.get_axis(1) == 0.82
    assert pad.get_axis(2) == -0.836
    assert pad.get_axis(3) == -0.803
    assert pad.get_axis(4) == (131 - 7) / 255
    assert pad.get_axis(5) == (255 - 4) / 255


def test_native_wup_pad_prefers_rust_melee_snapshot_axes_when_present():
    from NativeWupInput import NativeWupPad

    pad = NativeWupPad(0)
    pad.apply_state(
        {
            "connected": True,
            "raw_main_x": 128,
            "raw_main_y": 128,
            "raw_c_x": 128,
            "raw_c_y": 128,
            "left_trigger": 7,
            "right_trigger": 4,
            "melee": {
                "lstick_x": 127,
                "lstick_y": -128,
                "cstick_x": -128,
                "cstick_y": 127,
                "left_trigger": 42,
                "right_trigger": 201,
            },
        }
    )

    assert pad.get_axis(0) == 1.0
    assert pad.get_axis(1) == 1.0
    assert pad.get_axis(2) == -1.0
    assert pad.get_axis(3) == -1.0
    assert pad.get_axis(4) == 42 / 255
    assert pad.get_axis(5) == 201 / 255


def test_gather_inputs_uses_rust_melee_trigger_facts_for_native_wup_shield():
    from GatherInputs import gather_inputs
    from NativeWupInput import NativeWupPad

    player = PlayerStub()
    pad = NativeWupPad(0)
    pad.apply_state(
        {
            "connected": True,
            "raw_main_x": 128,
            "raw_main_y": 128,
            "raw_c_x": 128,
            "raw_c_y": 128,
            "left_trigger": 7,
            "right_trigger": 4,
        }
    )
    pad.apply_state(
        {
            "connected": True,
            "raw_main_x": 128,
            "raw_main_y": 128,
            "raw_c_x": 128,
            "raw_c_y": 128,
            "left_trigger": 71,
            "right_trigger": 4,
            "l": False,
            "r": False,
            "melee": {
                "lstick_x": 0,
                "lstick_y": 0,
                "cstick_x": 0,
                "cstick_y": 0,
                "left_trigger": 64,
                "right_trigger": 0,
                "left_trigger_analog_held": True,
                "right_trigger_analog_held": False,
            },
        }
    )

    gather_inputs(player, pad)

    assert player.l_trigger == trigger_after_deadzone(64 / 255)
    assert player.l_trigger_digital is False
    assert player.l_shieldkey is True
    assert player.l_shield_pressed is True
    assert player.r_shieldkey is False


def test_rust_melee_trigger_facts_below_deadzone_do_not_create_shield():
    from GatherInputs import gather_inputs
    from NativeWupInput import NativeWupPad

    player = PlayerStub()
    pad = NativeWupPad(0)
    pad.apply_state(
        {
            "connected": True,
            "raw_main_x": 128,
            "raw_main_y": 128,
            "raw_c_x": 128,
            "raw_c_y": 128,
            "left_trigger": 7,
            "right_trigger": 4,
        }
    )
    pad.apply_state(
        {
            "connected": True,
            "raw_main_x": 128,
            "raw_main_y": 128,
            "raw_c_x": 128,
            "raw_c_y": 128,
            "left_trigger": 49,
            "right_trigger": 4,
            "l": False,
            "r": False,
            "melee": {
                "lstick_x": 0,
                "lstick_y": 0,
                "cstick_x": 0,
                "cstick_y": 0,
                "left_trigger": 42,
                "right_trigger": 0,
                "left_trigger_analog_held": True,
                "right_trigger_analog_held": False,
            },
        }
    )

    gather_inputs(player, pad)

    assert player.l_trigger == 0
    assert player.l_shieldkey is False
    assert player.blockkey is False


def test_gather_inputs_uses_rust_melee_digital_trigger_edge_for_native_wup():
    from GatherInputs import gather_inputs
    from NativeWupInput import NativeWupPad

    player = PlayerStub()
    pad = NativeWupPad(0)
    pad.apply_state(
        {
            "connected": True,
            "raw_main_x": 128,
            "raw_main_y": 128,
            "raw_c_x": 128,
            "raw_c_y": 128,
            "left_trigger": 7,
            "right_trigger": 4,
        }
    )
    pad.apply_state(
        {
            "connected": True,
            "raw_main_x": 128,
            "raw_main_y": 128,
            "raw_c_x": 128,
            "raw_c_y": 128,
            "left_trigger": 7,
            "right_trigger": 255,
            "l": False,
            "r": False,
            "melee": {
                "lstick_x": 0,
                "lstick_y": 0,
                "cstick_x": 0,
                "cstick_y": 0,
                "left_trigger": 0,
                "right_trigger": 248,
                "left_trigger_analog_held": False,
                "right_trigger_analog_held": True,
                "left_trigger_digital_pressed": False,
                "right_trigger_digital_pressed": True,
            },
        }
    )

    gather_inputs(player, pad)

    assert player.r_shieldkey is True
    assert player.r_trigger_digital is True
    assert player.r_trigger_digital_pressed is True


def test_gather_inputs_uses_rust_melee_z_facts_without_trigger_airdodge():
    from GatherInputs import gather_inputs
    from NativeWupInput import NativeWupPad

    player = PlayerStub()
    pad = NativeWupPad(0)
    pad.apply_state(
        {
            "connected": True,
            "raw_main_x": 128,
            "raw_main_y": 128,
            "raw_c_x": 128,
            "raw_c_y": 128,
            "left_trigger": 0,
            "right_trigger": 0,
        }
    )
    pad.apply_state(
        {
            "connected": True,
            "raw_main_x": 128,
            "raw_main_y": 128,
            "raw_c_x": 128,
            "raw_c_y": 128,
            "left_trigger": 0,
            "right_trigger": 0,
            "z": True,
            "melee": {
                "lstick_x": 0,
                "lstick_y": 0,
                "cstick_x": 0,
                "cstick_y": 0,
                "left_trigger": 0,
                "right_trigger": 0,
                "attack_pressed": True,
                "grab_pressed": True,
                "shield_held": True,
                "shield_pressed": True,
                "digital_shield_pressed": False,
                "air_dodge_pressed": False,
                "left_trigger_digital_pressed": False,
                "right_trigger_digital_pressed": False,
            },
        }
    )

    gather_inputs(player, pad)

    assert player.akey is True
    assert player.grabkey is True
    assert player.melee_shield_held is True
    assert player.melee_shield_pressed is True
    assert player.melee_air_dodge_pressed is False
    assert player.blockkey is False
    assert player.l_trigger_digital_pressed is False
    assert player.r_trigger_digital_pressed is False


def test_gather_inputs_ignores_legacy_ucf_facts_and_uses_canonical_dash_fact():
    from GatherInputs import gather_inputs
    from NativeWupInput import NativeWupPad

    player = PlayerStub()
    pad = NativeWupPad(0)
    pad.apply_state(
        {
            "connected": True,
            "raw_main_x": 128,
            "raw_main_y": 128,
            "raw_c_x": 128,
            "raw_c_y": 128,
            "melee": {
                "lstick_x": -127,
                "lstick_y": 0,
                "cstick_x": 0,
                "cstick_y": 0,
                "dash_direction": 0,
                "ucf_dashback_direction": -1,
            },
        }
    )

    gather_inputs(player, pad)

    assert player.melee_dash_direction == 0
    assert not hasattr(player, "ucf_dashback_direction")


def test_native_wup_pad_recaptures_origin_after_gamecube_recenter_combo():
    from NativeWupInput import GAMECUBE_RECENTER_FRAMES, NativeWupPad

    pad = NativeWupPad(0)
    pad.apply_state(
        {
            "connected": True,
            "raw_main_x": 128,
            "raw_main_y": 128,
            "raw_c_x": 128,
            "raw_c_y": 128,
            "left_trigger": 0,
            "right_trigger": 0,
        }
    )

    for _ in range(GAMECUBE_RECENTER_FRAMES):
        pad.apply_state(
            {
                "connected": True,
                "raw_main_x": 140,
                "raw_main_y": 120,
                "raw_c_x": 132,
                "raw_c_y": 124,
                "left_trigger": 9,
                "right_trigger": 11,
                "x": True,
                "y": True,
                "start": True,
            }
        )

    assert pad.get_axis(0) == 0.0
    assert pad.get_axis(1) == 0.0
    assert pad.get_axis(2) == 0.0
    assert pad.get_axis(3) == 0.0
    assert pad.get_axis(4) == 0.0
    assert pad.get_axis(5) == 0.0


def test_native_wup_adapter_ignores_manual_calibration_file_for_native_origin_path(tmp_path):
    from NativeWupInput import NativeWupAdapter

    config_dir = tmp_path / "config"
    config_dir.mkdir()
    (config_dir / "gamecube_calibration.json").write_text(
        '{"main_x":{"min":40,"center":128,"max":220},'
        '"main_y":{"min":30,"center":128,"max":230},'
        '"c_x":{"min":50,"center":128,"max":210},'
        '"c_y":{"min":60,"center":128,"max":200}}',
        encoding="utf-8",
    )

    adapter = NativeWupAdapter(project_root=tmp_path)
    pad = adapter.pads[0]
    pad.apply_state(
        {
            "connected": True,
            "raw_main_x": 128,
            "raw_main_y": 128,
            "raw_c_x": 128,
            "raw_c_y": 128,
        }
    )
    pad.apply_state(
        {
            "connected": True,
            "raw_main_x": 220,
            "raw_main_y": 30,
            "raw_c_x": 50,
            "raw_c_y": 200,
        }
    )

    assert pad.get_axis(0) == 0.724
    assert pad.get_axis(1) == 0.766
    assert pad.get_axis(2) == -0.609
    assert pad.get_axis(3) == -0.567


def test_native_wup_pad_debug_lines_show_raw_values_and_native_origin_policy():
    from NativeWupInput import NativeWupPad, load_gamecube_calibration

    pad = NativeWupPad(0, calibration=load_gamecube_calibration())
    pad.apply_state(
        {
            "connected": True,
            "raw_main_x": 232,
            "raw_main_y": 128,
            "raw_c_x": 20,
            "raw_c_y": 128,
        }
    )

    lines = pad.debug_lines()

    assert "raw: main=(232,128) c=(20,128)" in lines
    assert "origin: main=(232,128) c=(20,128) triggers=(0,0)" in lines
    assert "native: console origin, no endpoint calibration" in lines


def test_native_wup_stream_line_updates_pad_state():
    from NativeWupInput import NativeWupPad, apply_stream_line

    pads = [NativeWupPad(0), NativeWupPad(1)]

    apply_stream_line(
        pads,
        '{"players":[{"connected":true,"source_port":2,"main_x":12000,"main_y":0,'
        '"c_x":0,"c_y":0,"left_trigger":0,"right_trigger":0,"a":false,"b":true,'
        '"x":false,"y":false,"z":false,"l":false,"r":false,"start":false,'
        '"dpad_up":false,"dpad_down":false,"dpad_left":false,"dpad_right":false},'
        '{"connected":false,"source_port":null,"main_x":0,"main_y":0,"c_x":0,"c_y":0,'
        '"left_trigger":0,"right_trigger":0,"a":false,"b":false,"x":false,"y":false,'
        '"z":false,"l":false,"r":false,"start":false,"dpad_up":false,'
        '"dpad_down":false,"dpad_left":false,"dpad_right":false}]}',
    )

    assert pads[0].connected()
    assert pads[0].get_button(1) is True
    assert pads[0].source_port == 2
    assert pads[1].connected() is False


def test_native_wup_adapter_uses_configured_runtime_exe(monkeypatch, tmp_path):
    from NativeWupInput import NativeWupAdapter

    runtime = tmp_path / "mole_runtime.exe"
    runtime.write_text("")
    monkeypatch.setenv("MOLE_RUNTIME_EXE", str(runtime))

    adapter = NativeWupAdapter(project_root=tmp_path)

    assert adapter._stream_command() == [str(runtime), "--stream-wup"]


def test_native_wup_adapter_prefers_dedicated_wup_runtime_over_default_target(tmp_path):
    from NativeWupInput import NativeWupAdapter

    stale_runtime = tmp_path / "target" / "debug" / "mole_runtime.exe"
    stale_runtime.parent.mkdir(parents=True)
    stale_runtime.write_text("")
    wup_runtime = tmp_path / "target-wup" / "debug" / "mole_runtime.exe"
    wup_runtime.parent.mkdir(parents=True)
    wup_runtime.write_text("")

    adapter = NativeWupAdapter(project_root=tmp_path)

    assert adapter._stream_command() == [str(wup_runtime), "--stream-wup"]


def test_native_wup_adapter_uses_featured_cargo_run_instead_of_default_target(
    monkeypatch, tmp_path
):
    import NativeWupInput
    from NativeWupInput import NativeWupAdapter

    stale_runtime = tmp_path / "target" / "debug" / "mole_runtime.exe"
    stale_runtime.parent.mkdir(parents=True)
    stale_runtime.write_text("")
    monkeypatch.setattr(NativeWupInput.shutil, "which", lambda name: "cargo.exe")

    adapter = NativeWupAdapter(project_root=tmp_path)

    assert adapter._stream_command() == [
        "cargo.exe",
        "run",
        "-q",
        "-p",
        "mole_runtime",
        "--features",
        "wup",
        "--",
        "--stream-wup",
    ]


def test_native_wup_adapter_clears_pads_when_stream_ends(tmp_path):
    from NativeWupInput import NativeWupAdapter

    class FakeProcess:
        stdout = iter(
            [
                '{"players":[{"connected":true,"source_port":1,'
                '"raw_main_x":200,"raw_main_y":128,"raw_c_x":128,"raw_c_y":128,'
                '"main_x":18432,"main_y":0,"c_x":0,"c_y":0,'
                '"left_trigger":0,"right_trigger":0,"a":false,"b":false,'
                '"x":false,"y":false,"z":false,"l":false,"r":false,'
                '"start":false,"dpad_up":false,"dpad_down":false,'
                '"dpad_left":false,"dpad_right":false}]}'
            ]
        )

    adapter = NativeWupAdapter(project_root=tmp_path)
    adapter._process = FakeProcess()

    adapter._read_loop()

    assert adapter.pads[0].connected() is False
    assert adapter.pads[0].get_axis(0) == 0


def test_native_wup_adapter_does_not_reopen_stream_after_eof(monkeypatch, tmp_path):
    from NativeWupInput import NativeWupAdapter

    connected_line = (
        '{"players":[{"connected":true,"source_port":1,'
        '"raw_main_x":200,"raw_main_y":128,"raw_c_x":128,"raw_c_y":128,'
        '"main_x":18432,"main_y":0,"c_x":0,"c_y":0,'
        '"left_trigger":0,"right_trigger":0,"a":false,"b":false,'
        '"x":false,"y":false,"z":false,"l":false,"r":false,'
        '"start":false,"dpad_up":false,"dpad_down":false,'
        '"dpad_left":false,"dpad_right":false}]}'
    )

    class FakeProcess:
        def __init__(self, lines):
            self.stdout = iter(lines)

        def poll(self):
            return 0

        def wait(self, timeout=None):
            return 0

    adapter = NativeWupAdapter(project_root=tmp_path)
    opens = []

    def fake_open_stream_process():
        if opens:
            raise AssertionError("stream EOF should not recursively reopen in Python")
        opens.append(len(opens))
        return FakeProcess([connected_line])

    monkeypatch.setattr(adapter, "_open_stream_process", fake_open_stream_process)
    monkeypatch.setattr(adapter, "_stream_command", lambda: ["runtime", "--stream-wup"])

    adapter._stream_loop()

    assert len(opens) == 1


def test_native_wup_adapter_stop_waits_for_stream_process(tmp_path):
    from NativeWupInput import NativeWupAdapter

    class FakeProcess:
        def __init__(self):
            self.terminated = False
            self.killed = False
            self.wait_timeouts = []
            self._poll = None

        def poll(self):
            return self._poll

        def terminate(self):
            self.terminated = True

        def wait(self, timeout=None):
            self.wait_timeouts.append(timeout)
            self._poll = 0

        def kill(self):
            self.killed = True
            self._poll = -9

    adapter = NativeWupAdapter(project_root=tmp_path)
    process = FakeProcess()
    adapter._process = process

    adapter.stop()

    assert process.terminated is True
    assert process.wait_timeouts == [1.0]
    assert process.killed is False
    assert adapter._process is None


def test_native_wup_adapter_logs_helper_exit_code(tmp_path):
    from NativeWupInput import NativeWupAdapter

    class FakeProcess:
        def poll(self):
            return 7

        def wait(self, timeout=None):
            return 7

    adapter = NativeWupAdapter(project_root=tmp_path)
    log_dir = tmp_path / "logs"
    log_dir.mkdir()
    log_path = log_dir / "wup-helper-test.log"
    adapter._stderr_log_path = log_path
    adapter._stderr_handle = log_path.open("w", encoding="utf-8")

    adapter._finish_stream_process(FakeProcess())

    assert "WUP helper exited with code 7" in log_path.read_text(encoding="utf-8")


def test_debug_overlay_lines_show_active_source_and_inputs():
    from DebugOverlay import build_debug_lines

    player = PlayerStub()
    player.main_stick = [0.75, -0.25]
    player.c_stick = [-0.5, 0.125]
    player.l_trigger = 0.25
    player.r_trigger = 1.0
    player.akey = True
    player.specialkey = False
    player.jumpkey = True
    player.grabkey = False
    player.blockkey = True
    player.menukey = False
    player.upkey = True
    player.downkey = False
    player.leftkey = False
    player.rightkey = True
    player.state = "dashing"
    player.x = 101.25
    player.y = 202.5
    player.xVelocity = 3.5
    player.yVelocity = -1.25
    player.grounded = True
    player.actionable = False
    player.isRight = True
    player.xCount = 2
    player.dashCount = 4
    player.turnCount = 0
    player.canDash = False
    player.canJump = True
    player.canBlock = False

    lines = build_debug_lines("P1", player, FakeJoystick("Native WUP-028 Port 2"))

    assert "P1 source: Native WUP-028 Port 2" in lines
    assert "main: +0.750, -0.250  c: -0.500, +0.125" in lines
    assert "triggers: L +0.250  R +1.000" in lines
    assert "buttons: A=1 B=0 J=1 G=0 Z=1 M=0" in lines
    assert "dpad: U=1 D=0 L=0 R=1" in lines
    assert "state: dashing  pos: +101.250, +202.500  vel: +3.500, -1.250" in lines
    assert "flags: grounded=1 actionable=0 facing=R" in lines
    assert "counts: x=2 dash=4 turn=0  can: dash=0 jump=1 block=0" in lines


def test_debug_overlay_includes_native_source_diagnostics():
    from DebugOverlay import build_debug_lines

    class DiagnosticSource(FakeJoystick):
        def debug_lines(self):
            return [
                "raw: main=(232,128) c=(20,128)",
                "native: console origin, no endpoint calibration",
            ]

    player = PlayerStub()
    player.main_stick = [1.0, 0.0]
    player.c_stick = [-1.0, 0.0]

    lines = build_debug_lines("P1", player, DiagnosticSource())

    assert "raw: main=(232,128) c=(20,128)" in lines
    assert "native: console origin, no endpoint calibration" in lines
