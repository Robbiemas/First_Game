import os
import sys
import time
import traceback
from datetime import datetime
from pathlib import Path

from tools.process_lock import acquire_instance_lock


PROJECT_ROOT = Path(__file__).resolve().parent
GAME_ALREADY_RUNNING_EXIT_CODE = 2


def configure_sdl_controller_hints():
    os.environ.setdefault("SDL_JOYSTICK_HIDAPI", "1")
    os.environ.setdefault("SDL_JOYSTICK_HIDAPI_GAMECUBE", "1")


configure_sdl_controller_hints()

import pygame

from Characters import Character
from ChooseAction import resolve_action_state
from DebugOverlay import DebugInputLogger, draw_debug_overlay
from DisplayInputs import disp_fps, text_objects
from GatherInputs import gather_inputs
from NativeWupInput import create_native_wup_adapter
from stages import Stage
from states import MainMenu, Pause, START_GAME


DEFAULT_WINDOW_SIZE = [1280, 720]
FPS = 60
LEGACY_HARNESS_CAPTION = "Mole Game - Legacy Pygame QA Harness (Rust runtime authoritative)"


def env_flag(name, default=False):
    value = os.environ.get(name)
    if value is None:
        return default
    return value.lower() in {"1", "true", "yes", "on"}


def read_max_frames(max_frames=None):
    if max_frames is not None:
        return max_frames
    value = os.environ.get("MOLE_MAX_FRAMES")
    if not value:
        return None
    return int(value)


def write_crash_log(exc, log_dir=None):
    log_dir = Path(log_dir or PROJECT_ROOT / "logs")
    log_dir.mkdir(parents=True, exist_ok=True)
    stamp = datetime.now().strftime("%Y%m%d-%H%M%S-%f")
    path = log_dir / f"crash-{stamp}.log"
    traceback_text = "".join(
        traceback.format_exception(type(exc), exc, exc.__traceback__)
    )
    path.write_text(f"Mole Game crash\n\n{traceback_text}", encoding="utf-8")
    return path


def should_show_main_menu(max_frames):
    return max_frames is None and env_flag("MOLE_SHOW_MENU")


def game_instance_lock_path(project_root=PROJECT_ROOT):
    return Path(project_root).resolve() / "logs" / "mole-game.lock"


def acquire_game_instance_lock(project_root=PROJECT_ROOT):
    return acquire_instance_lock(game_instance_lock_path(project_root))


def run_with_single_instance(max_frames=None):
    game_lock = acquire_game_instance_lock()
    if game_lock is None:
        print("Mole Game is already running. Close the existing game window first.", file=sys.stderr)
        return GAME_ALREADY_RUNNING_EXIT_CODE

    with game_lock:
        try:
            gameloop(max_frames)
            return 0
        except KeyboardInterrupt:
            pygame.quit()
            return 130
        except Exception as exc:
            crash_log = write_crash_log(exc)
            traceback.print_exception(type(exc), exc, exc.__traceback__)
            print(f"Crash log: {crash_log}", file=sys.stderr)
            pygame.quit()
            return 1


def init_joysticks(pygame_module=pygame):
    pygame_module.joystick.init()
    joysticks = []
    for index in range(pygame_module.joystick.get_count()):
        joystick = pygame_module.joystick.Joystick(index)
        joystick.init()
        joysticks.append(joystick)
    return joysticks


def create_window():
    pygame.display.set_caption(LEGACY_HARNESS_CAPTION)
    fullscreen = env_flag("MOLE_FULLSCREEN")
    if fullscreen:
        monitor_size = [
            pygame.display.Info().current_w,
            pygame.display.Info().current_h,
        ]
        flags = pygame.FULLSCREEN | pygame.HWSURFACE | pygame.DOUBLEBUF
        return pygame.display.set_mode(monitor_size, flags), monitor_size, fullscreen

    monitor_size = [
        int(os.environ.get("MOLE_WIDTH", DEFAULT_WINDOW_SIZE[0])),
        int(os.environ.get("MOLE_HEIGHT", DEFAULT_WINDOW_SIZE[1])),
    ]
    return pygame.display.set_mode(monitor_size, pygame.RESIZABLE), monitor_size, fullscreen


def message_display(win, monitor_size, text):
    large_text = pygame.font.Font("freesansbold.ttf", 115)
    text_surface, text_rect = text_objects(text, large_text)
    text_rect.center = (monitor_size[0] / 2, monitor_size[1] / 2)
    win.blit(text_surface, text_rect)
    pygame.display.update()
    time.sleep(2)


def player_input_sources(joysticks):
    player1_source = joysticks[0] if len(joysticks) >= 1 else None
    player2_source = joysticks[1] if len(joysticks) >= 2 else None
    keyboard_player1 = not env_flag("MOLE_DISABLE_KEYBOARD")
    return player1_source, player2_source, keyboard_player1


def run_main_menu(win, monitor_size):
    return MainMenu(win, monitor_size).run()


def gameloop(max_frames=None):
    pygame.init()
    clock = pygame.time.Clock()
    true_scroll = [0, 0]
    frame_passed = 0
    initialized = False

    win, monitor_size, fullscreen = create_window()
    max_frames = read_max_frames(max_frames)
    if should_show_main_menu(max_frames) and run_main_menu(win, monitor_size) != START_GAME:
        pygame.quit()
        return

    joysticks = init_joysticks()
    native_wup = create_native_wup_adapter()
    player1_source, player2_source, keyboard_player1 = player_input_sources(joysticks)
    frames_run = 0
    debug_inputs = env_flag("MOLE_DEBUG_INPUTS")
    debug_logger = DebugInputLogger(env_flag("MOLE_DEBUG_INPUT_LOG"))

    run = True
    stage_selection = Stage("first")
    platforms = stage_selection.load_platforms()
    players = []
    has_imported_camera = False

    player1 = Character(stage_selection.spawn_position(1)[0], stage_selection.spawn_position(1)[1], players)
    player1.spawn = stage_selection.spawn_position(1)
    player2 = Character(stage_selection.spawn_position(2)[0], stage_selection.spawn_position(2)[1], players)
    player2.spawn = stage_selection.spawn_position(2)

    player1.choosechar("DolphinMole")
    player2.choosechar("DolphinMole")

    try:
        while run:
            clock.tick_busy_loop(FPS)
            win.fill([255, 255, 255])

            if frame_passed < 1:
                frame_passed += 1
            else:
                initialized = True
                if not has_imported_camera:
                    import Camera

                    Camera.grab_images()
                    has_imported_camera = True

            for event in pygame.event.get():
                if event.type == pygame.QUIT:
                    run = False
                if event.type == pygame.VIDEORESIZE and not fullscreen:
                    win = pygame.display.set_mode((event.w, event.h), pygame.RESIZABLE)
                    monitor_size = [event.w, event.h]

                if event.type == pygame.KEYDOWN:
                    if event.key == pygame.K_ESCAPE:
                        run = False
                    if event.key == pygame.K_F3:
                        debug_inputs = not debug_inputs
                    if event.key == pygame.K_F4:
                        debug_logger.toggle()
                    if event.key == pygame.K_f:
                        fullscreen = not fullscreen
                        if fullscreen:
                            monitor_size = [
                                pygame.display.Info().current_w,
                                pygame.display.Info().current_h,
                            ]
                            win = pygame.display.set_mode(monitor_size, pygame.FULLSCREEN)
                        else:
                            monitor_size = DEFAULT_WINDOW_SIZE[:]
                            win = pygame.display.set_mode(monitor_size, pygame.RESIZABLE)

            active_player1_source = player1_source
            active_player2_source = player2_source
            if native_wup is not None:
                wup_player1_source, wup_player2_source = native_wup.input_sources()
                active_player1_source = wup_player1_source or active_player1_source
                active_player2_source = wup_player2_source or active_player2_source
            active_input_sources = [active_player1_source, active_player2_source]

            keys = pygame.key.get_pressed()
            gather_inputs(player1, active_player1_source, keys=keys, keyboard=keyboard_player1)
            gather_inputs(player2, active_player2_source)

            player1.player_collision(player2)
            player2.player_collision(player1)

            for play in players:
                play.reset_ground()
                current_platform = None
                for plat in platforms:
                    if play.collision_check(plat):
                        play.offset(play.x, plat.y)
                        play.is_grounded()
                        current_platform = plat
                if play.grounded and current_platform is not None:
                    if (
                        current_platform.solid
                        or play.main_stick[1] < 0.55
                        or play.dropCount >= 5
                        or not play.actionable
                    ):
                        play.changeY(current_platform.plat_y() - 1)

            xpos = 100
            if initialized:
                scroll = Camera.camera_adjust(player1, player2, monitor_size, true_scroll)
                Camera.draw_bg(win, scroll)

                for play in players:
                    resolve_action_state(play)
                    Camera.get_mask(play)
                    play.set_prev_cords()
                    play.move_x()
                    play.move_y()
                    play.check_death(stage_selection)

                for play in players:
                    if play.is_dead():
                        print("A player ran out of lives.")
                        play.new_game()
                    Camera.draw_prev_ecb(win, play, scroll)
                    Camera.draw_char(win, play, xpos, scroll)
                    if play.menukey and play.menu:
                        play.menu = False
                        Pause(
                            win,
                            monitor_size,
                            active_player1_source,
                            play,
                            scroll,
                            platforms,
                            xpos,
                        )

                    xpos += 650

                for platform in platforms:
                    Camera.draw_stage(win, platform, scroll)

            debug_logger.write_frame(frames_run, [player1, player2], active_input_sources)
            if debug_inputs:
                draw_debug_overlay(win, [player1, player2], active_input_sources, logger=debug_logger)

            disp_fps(win, clock)
            pygame.display.flip()

            frames_run += 1
            if max_frames is not None and frames_run >= max_frames:
                run = False
    finally:
        if native_wup is not None:
            native_wup.stop()
        debug_logger.close()
        pygame.quit()


if __name__ == "__main__":
    sys.exit(run_with_single_instance())
