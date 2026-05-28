import os
import sys
import time


def configure_sdl_controller_hints():
    os.environ.setdefault("SDL_JOYSTICK_HIDAPI", "1")
    os.environ.setdefault("SDL_JOYSTICK_HIDAPI_GAMECUBE", "1")


configure_sdl_controller_hints()

import pygame


def rounded_axes(joystick):
    return [round(joystick.get_axis(index), 2) for index in range(joystick.get_numaxes())]


def pressed_buttons(joystick):
    return [index for index in range(joystick.get_numbuttons()) if joystick.get_button(index)]


def hats(joystick):
    return [joystick.get_hat(index) for index in range(joystick.get_numhats())]


def snapshot(joystick):
    return {
        "axes": rounded_axes(joystick),
        "buttons": pressed_buttons(joystick),
        "hats": hats(joystick),
    }


def print_device(index, joystick):
    print(f"[{index}] {joystick.get_name()}")
    print(
        f"    axes={joystick.get_numaxes()} buttons={joystick.get_numbuttons()} "
        f"hats={joystick.get_numhats()}"
    )


def watch_devices(joysticks, seconds=20):
    print()
    print(f"Watching input for {seconds} seconds. Move sticks and press buttons now.")
    previous = {}
    deadline = time.time() + seconds

    while time.time() < deadline:
        pygame.event.pump()
        for index, joystick in enumerate(joysticks):
            current = snapshot(joystick)
            if previous.get(index) != current:
                previous[index] = current
                print(
                    f"[{index}] axes={current['axes']} "
                    f"buttons={current['buttons']} hats={current['hats']}"
                )
        time.sleep(0.08)


def main():
    pygame.init()
    pygame.joystick.init()

    print(f"pygame {pygame.version.ver}")
    print(f"SDL {'.'.join(str(part) for part in pygame.get_sdl_version())}")
    print(f"SDL_JOYSTICK_HIDAPI={os.environ.get('SDL_JOYSTICK_HIDAPI')}")
    print(f"SDL_JOYSTICK_HIDAPI_GAMECUBE={os.environ.get('SDL_JOYSTICK_HIDAPI_GAMECUBE')}")
    print()

    joysticks = []
    for index in range(pygame.joystick.get_count()):
        joystick = pygame.joystick.Joystick(index)
        joystick.init()
        joysticks.append(joystick)
        print_device(index, joystick)

    if not joysticks:
        print("No SDL/Pygame joysticks were detected.")
        pygame.quit()
        return 1

    if "--watch" in sys.argv:
        watch_devices(joysticks)

    pygame.quit()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
