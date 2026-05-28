import importlib
import sys

import pygame
import pytest


def fresh_main_module(monkeypatch):
    monkeypatch.setenv("SDL_VIDEODRIVER", "dummy")
    sys.modules.pop("RealMainFile", None)
    return importlib.import_module("RealMainFile")


def test_main_menu_start_button_click_requests_game_start(monkeypatch):
    monkeypatch.setenv("SDL_VIDEODRIVER", "dummy")
    pygame.init()
    try:
        from states import MainMenu

        display = pygame.Surface((1280, 720))
        menu = MainMenu(display, [1280, 720])
        click = pygame.event.Event(
            pygame.MOUSEBUTTONDOWN,
            {"button": 1, "pos": menu.start_button_rect.center},
        )

        assert menu.handle_event(click) == "start_game"
    finally:
        pygame.quit()


def test_gameloop_returns_before_gameplay_when_enabled_main_menu_quits(monkeypatch):
    module = fresh_main_module(monkeypatch)
    monkeypatch.setenv("MOLE_SHOW_MENU", "1")
    pygame.init()

    monkeypatch.setattr(
        module,
        "create_window",
        lambda: (pygame.Surface((1280, 720)), [1280, 720], False),
    )
    monkeypatch.setattr(module, "run_main_menu", lambda _win, _monitor_size: "quit", raising=False)
    monkeypatch.setattr(module, "init_joysticks", lambda: [])
    monkeypatch.setattr(module, "create_native_wup_adapter", lambda: None)

    class StageShouldNotLoad:
        def __init__(self, _name):
            raise AssertionError("gameplay should not load until the menu starts the game")

    monkeypatch.setattr(module, "Stage", StageShouldNotLoad)

    try:
        module.gameloop()
    except AssertionError:
        pytest.fail("gameloop loaded gameplay after the main menu requested quit")
    finally:
        pygame.quit()


def test_frame_limited_smoke_runs_skip_interactive_main_menu(monkeypatch):
    module = fresh_main_module(monkeypatch)

    assert module.should_show_main_menu(None) is False
    monkeypatch.setenv("MOLE_SHOW_MENU", "1")
    assert module.should_show_main_menu(None) is True
    assert module.should_show_main_menu(2) is False


def test_legacy_pygame_window_caption_marks_harness_status(monkeypatch):
    module = fresh_main_module(monkeypatch)
    monkeypatch.setenv("MOLE_WIDTH", "320")
    monkeypatch.setenv("MOLE_HEIGHT", "180")
    captions = []

    monkeypatch.setattr(
        module.pygame.display,
        "set_caption",
        lambda caption: captions.append(caption),
    )
    monkeypatch.setattr(
        module.pygame.display,
        "set_mode",
        lambda size, _flags=0: pygame.Surface(size),
    )

    module.create_window()

    assert captions == [module.LEGACY_HARNESS_CAPTION]
    assert "Legacy Pygame QA Harness" in captions[0]
    assert "Rust runtime authoritative" in captions[0]


def test_write_crash_log_records_traceback(monkeypatch, tmp_path):
    module = fresh_main_module(monkeypatch)

    try:
        raise RuntimeError("controller crash sample")
    except RuntimeError as exc:
        path = module.write_crash_log(exc, log_dir=tmp_path)

    text = path.read_text(encoding="utf-8")
    assert path.parent == tmp_path
    assert path.name.startswith("crash-")
    assert "RuntimeError: controller crash sample" in text
    assert "test_write_crash_log_records_traceback" in text


def test_game_instance_lock_rejects_second_holder(monkeypatch, tmp_path):
    module = fresh_main_module(monkeypatch)

    first_lock = module.acquire_game_instance_lock(tmp_path)
    assert first_lock is not None

    try:
        assert module.acquire_game_instance_lock(tmp_path) is None
    finally:
        first_lock.release()

    second_lock = module.acquire_game_instance_lock(tmp_path)
    assert second_lock is not None
    second_lock.release()
