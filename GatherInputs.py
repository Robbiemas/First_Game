import pygame


MAIN_STICK_DEADZONE = 0.05
TRIGGER_DEADZONE = 0.20


def _button(joystick, index):
    if joystick is None or joystick.get_numbuttons() <= index:
        return False
    return bool(joystick.get_button(index))


def _axis(joystick, index):
    if joystick is None or joystick.get_numaxes() <= index:
        return 0
    return joystick.get_axis(index)


def _trigger_axis(joystick, index):
    value = max(0, min(_axis(joystick, index), 1))
    if value <= TRIGGER_DEADZONE:
        return 0
    return (value - TRIGGER_DEADZONE) / (1 - TRIGGER_DEADZONE)


def _trigger_held(value):
    return value > 0


def _melee_fact(joystick, name, default=None):
    if joystick is None or not hasattr(joystick, "get_melee_fact"):
        return default
    return joystick.get_melee_fact(name, default)


def _melee_optional_int_fact(joystick, name):
    value = _melee_fact(joystick, name)
    if value is None:
        return None
    try:
        return int(value)
    except (TypeError, ValueError):
        return None


def _controller_name(joystick):
    if joystick is None or not hasattr(joystick, "get_name"):
        return ""
    return joystick.get_name().lower()


def _is_xbox_controller(joystick):
    name = _controller_name(joystick)
    return "xbox" in name or "xinput" in name


def _is_gamecube_controller(joystick):
    name = _controller_name(joystick)
    return any(
        marker in name
        for marker in ("gamecube", "game cube", "wup", "mayflash", "gc adapter", "nintendo")
    )


def _pressed(keys, key):
    return bool(keys is not None and keys[key])


def _combine_axis(primary, fallback):
    return primary if primary != 0 else fallback


def _apply_deadzone(player):
    if abs(player.main_stick[0]) < MAIN_STICK_DEADZONE:
        player.main_stick[0] = 0
    if abs(player.main_stick[1]) < MAIN_STICK_DEADZONE:
        player.main_stick[1] = 0


def _set_shield_state(
    player,
    *,
    left_shield,
    right_shield,
    left_digital=False,
    right_digital=False,
    left_digital_pressed=None,
    right_digital_pressed=None,
):
    previous_left = bool(getattr(player, "l_shieldkey", False))
    previous_right = bool(getattr(player, "r_shieldkey", False))
    previous_block = bool(getattr(player, "blockkey", False))
    previous_left_digital = bool(getattr(player, "l_trigger_digital", False))
    previous_right_digital = bool(getattr(player, "r_trigger_digital", False))

    player.l_shieldkey = bool(left_shield)
    player.r_shieldkey = bool(right_shield)
    player.l_shield_pressed = player.l_shieldkey and not previous_left
    player.r_shield_pressed = player.r_shieldkey and not previous_right
    player.l_trigger_digital = bool(left_digital)
    player.r_trigger_digital = bool(right_digital)
    player.l_trigger_digital_pressed = (
        bool(left_digital_pressed)
        if left_digital_pressed is not None
        else player.l_trigger_digital and not previous_left_digital
    )
    player.r_trigger_digital_pressed = (
        bool(right_digital_pressed)
        if right_digital_pressed is not None
        else player.r_trigger_digital and not previous_right_digital
    )
    player.blockkey = player.l_shieldkey or player.r_shieldkey
    player.block_pressed = player.blockkey and not previous_block

    if not player.blockkey:
        player.canBlock = True


def _merge_shield_state(
    player,
    *,
    left_shield,
    right_shield,
    left_digital=False,
    right_digital=False,
    left_digital_pressed=None,
    right_digital_pressed=None,
):
    previous_left = bool(getattr(player, "l_shieldkey", False))
    previous_right = bool(getattr(player, "r_shieldkey", False))
    previous_block = bool(getattr(player, "blockkey", False))
    previous_left_digital = bool(getattr(player, "l_trigger_digital", False))
    previous_right_digital = bool(getattr(player, "r_trigger_digital", False))

    player.l_shieldkey = previous_left or bool(left_shield)
    player.r_shieldkey = previous_right or bool(right_shield)
    player.l_shield_pressed = bool(getattr(player, "l_shield_pressed", False)) or (
        bool(left_shield) and not previous_left
    )
    player.r_shield_pressed = bool(getattr(player, "r_shield_pressed", False)) or (
        bool(right_shield) and not previous_right
    )
    player.l_trigger_digital = previous_left_digital or bool(left_digital)
    player.r_trigger_digital = previous_right_digital or bool(right_digital)
    player.l_trigger_digital_pressed = bool(
        getattr(player, "l_trigger_digital_pressed", False)
    ) or (
        bool(left_digital_pressed)
        if left_digital_pressed is not None
        else bool(left_digital) and not previous_left_digital
    )
    player.r_trigger_digital_pressed = bool(
        getattr(player, "r_trigger_digital_pressed", False)
    ) or (
        bool(right_digital_pressed)
        if right_digital_pressed is not None
        else bool(right_digital) and not previous_right_digital
    )
    player.blockkey = player.l_shieldkey or player.r_shieldkey
    player.block_pressed = bool(getattr(player, "block_pressed", False)) or (
        player.blockkey and not previous_block
    )

    if not player.blockkey:
        player.canBlock = True


def _set_button_state(
    player,
    *,
    akey,
    specialkey,
    jumpkey,
    grabkey,
    menukey,
    left_shield=False,
    right_shield=False,
    left_digital=False,
    right_digital=False,
    left_digital_pressed=None,
    right_digital_pressed=None,
    melee_attack_pressed=None,
    melee_special_pressed=None,
    melee_grab_pressed=None,
    melee_shield_held=None,
    melee_shield_pressed=None,
    melee_air_dodge_pressed=None,
    melee_x_tap_timer=None,
    melee_y_tap_timer=None,
    melee_dash_direction=None,
):
    player.akey = bool(akey)
    player.specialkey = bool(specialkey)
    player.jumpkey = bool(jumpkey)
    player.grabkey = bool(grabkey)
    player.menukey = bool(menukey)
    _set_shield_state(
        player,
        left_shield=left_shield,
        right_shield=right_shield,
        left_digital=left_digital,
        right_digital=right_digital,
        left_digital_pressed=left_digital_pressed,
        right_digital_pressed=right_digital_pressed,
    )
    player.melee_attack_pressed = (
        player.akey if melee_attack_pressed is None else bool(melee_attack_pressed)
    )
    player.melee_special_pressed = (
        player.specialkey if melee_special_pressed is None else bool(melee_special_pressed)
    )
    player.melee_grab_pressed = (
        player.grabkey if melee_grab_pressed is None else bool(melee_grab_pressed)
    )
    player.melee_shield_held = (
        player.blockkey if melee_shield_held is None else bool(melee_shield_held)
    )
    player.melee_shield_pressed = (
        player.block_pressed if melee_shield_pressed is None else bool(melee_shield_pressed)
    )
    player.melee_air_dodge_pressed = (
        player.l_trigger_digital_pressed or player.r_trigger_digital_pressed
        if melee_air_dodge_pressed is None
        else bool(melee_air_dodge_pressed)
    )
    player.melee_x_tap_timer = melee_x_tap_timer
    player.melee_y_tap_timer = melee_y_tap_timer
    player.melee_dash_direction = (
        None if melee_dash_direction is None else int(melee_dash_direction)
    )
    if not player.jumpkey:
        if getattr(player, "jumpSquat", False):
            player.jump_released_during_squat = True
        else:
            player.canJump = True
    if not player.grabkey:
        player.releasePause = True
    if not player.menukey:
        player.menu = True


def _gather_keyboard(player, keys):
    x_axis = 0
    y_axis = 0

    if _pressed(keys, pygame.K_a) or _pressed(keys, pygame.K_LEFT):
        x_axis -= 1
    if _pressed(keys, pygame.K_d) or _pressed(keys, pygame.K_RIGHT):
        x_axis += 1
    if _pressed(keys, pygame.K_w) or _pressed(keys, pygame.K_UP):
        y_axis -= 1
    if _pressed(keys, pygame.K_s) or _pressed(keys, pygame.K_DOWN):
        y_axis += 1

    player.main_stick = [x_axis, y_axis]
    player.c_stick = [0, 0]
    player.r_trigger = 0
    player.l_trigger = 0
    player.upkey = _pressed(keys, pygame.K_w) or _pressed(keys, pygame.K_UP)
    player.downkey = _pressed(keys, pygame.K_s) or _pressed(keys, pygame.K_DOWN)
    player.leftkey = _pressed(keys, pygame.K_a) or _pressed(keys, pygame.K_LEFT)
    player.rightkey = _pressed(keys, pygame.K_d) or _pressed(keys, pygame.K_RIGHT)

    left_shield = _pressed(keys, pygame.K_l)
    right_shield = _pressed(keys, pygame.K_LSHIFT)

    _set_button_state(
        player,
        akey=_pressed(keys, pygame.K_j) or _pressed(keys, pygame.K_SPACE),
        specialkey=_pressed(keys, pygame.K_u),
        jumpkey=_pressed(keys, pygame.K_k)
        or _pressed(keys, pygame.K_w)
        or _pressed(keys, pygame.K_UP),
        grabkey=_pressed(keys, pygame.K_i),
        menukey=_pressed(keys, pygame.K_p) or _pressed(keys, pygame.K_RETURN),
        left_shield=left_shield,
        right_shield=right_shield,
        left_digital=left_shield,
        right_digital=right_shield,
    )


def _gather_idle(player):
    player.main_stick = [0, 0]
    player.c_stick = [0, 0]
    player.r_trigger = 0
    player.l_trigger = 0
    player.upkey = False
    player.downkey = False
    player.leftkey = False
    player.rightkey = False
    _set_button_state(
        player,
        akey=False,
        specialkey=False,
        jumpkey=False,
        grabkey=False,
        menukey=False,
        left_shield=False,
        right_shield=False,
    )


def _gather_joystick(player, joystick):
    player.main_stick = [_axis(joystick, 0), _axis(joystick, 1)]

    if _is_xbox_controller(joystick):
        player.c_stick = [_axis(joystick, 3), _axis(joystick, 4)]
        player.l_trigger = _trigger_axis(joystick, 2)
        player.r_trigger = _trigger_axis(joystick, 5)
        left_digital = _button(joystick, 4)
        right_digital = _button(joystick, 5)
        left_shield = left_digital or _trigger_held(player.l_trigger)
        right_shield = right_digital or _trigger_held(player.r_trigger)
    elif _is_gamecube_controller(joystick):
        player.c_stick = [_axis(joystick, 2), _axis(joystick, 3)]
        player.l_trigger = _trigger_axis(joystick, 4)
        player.r_trigger = _trigger_axis(joystick, 5)
        left_digital = _button(joystick, 5)
        right_digital = _button(joystick, 6)
        left_shield = left_digital or _trigger_held(player.l_trigger)
        right_shield = right_digital or _trigger_held(player.r_trigger)
    else:
        player.c_stick = [_axis(joystick, 5), _axis(joystick, 4)]
        player.r_trigger = _trigger_axis(joystick, 3)
        player.l_trigger = _trigger_axis(joystick, 2)
        left_digital = _button(joystick, 5)
        right_digital = _button(joystick, 6)
        left_shield = left_digital or _trigger_held(player.l_trigger)
        right_shield = right_digital or _trigger_held(player.r_trigger)

    left_analog_held = _melee_fact(joystick, "left_trigger_analog_held")
    right_analog_held = _melee_fact(joystick, "right_trigger_analog_held")
    left_digital_pressed = _melee_fact(joystick, "left_trigger_digital_pressed")
    right_digital_pressed = _melee_fact(joystick, "right_trigger_digital_pressed")
    melee_attack_pressed = _melee_fact(joystick, "attack_pressed")
    melee_special_pressed = _melee_fact(joystick, "special_pressed")
    melee_grab_pressed = _melee_fact(joystick, "grab_pressed")
    melee_shield_held = _melee_fact(joystick, "shield_held")
    melee_shield_pressed = _melee_fact(joystick, "shield_pressed")
    melee_air_dodge_pressed = _melee_fact(joystick, "air_dodge_pressed")
    melee_x_tap_timer = _melee_optional_int_fact(joystick, "x_tap_timer")
    melee_y_tap_timer = _melee_optional_int_fact(joystick, "y_tap_timer")
    melee_dash_direction = _melee_optional_int_fact(joystick, "dash_direction")
    if left_analog_held is not None:
        left_shield = left_digital or (
            bool(left_analog_held) and _trigger_held(player.l_trigger)
        )
    if right_analog_held is not None:
        right_shield = right_digital or (
            bool(right_analog_held) and _trigger_held(player.r_trigger)
        )
    if left_digital_pressed is not None:
        left_digital_pressed = bool(left_digital_pressed) and _trigger_held(player.l_trigger)
        left_digital = left_digital or left_digital_pressed
        left_shield = left_shield or bool(left_digital_pressed)
    if right_digital_pressed is not None:
        right_digital_pressed = bool(right_digital_pressed) and _trigger_held(player.r_trigger)
        right_digital = right_digital or right_digital_pressed
        right_shield = right_shield or bool(right_digital_pressed)

    _apply_deadzone(player)

    _set_button_state(
        player,
        akey=_button(joystick, 0)
        or (melee_attack_pressed is not None and bool(melee_attack_pressed)),
        specialkey=_button(joystick, 1)
        or (melee_special_pressed is not None and bool(melee_special_pressed)),
        jumpkey=_button(joystick, 2) or _button(joystick, 3),
        grabkey=_button(joystick, 4)
        or (melee_grab_pressed is not None and bool(melee_grab_pressed)),
        menukey=_button(joystick, 7),
        left_shield=left_shield,
        right_shield=right_shield,
        left_digital=left_digital,
        right_digital=right_digital,
        left_digital_pressed=left_digital_pressed,
        right_digital_pressed=right_digital_pressed,
        melee_attack_pressed=melee_attack_pressed,
        melee_special_pressed=melee_special_pressed,
        melee_grab_pressed=melee_grab_pressed,
        melee_shield_held=melee_shield_held,
        melee_shield_pressed=melee_shield_pressed,
        melee_air_dodge_pressed=melee_air_dodge_pressed,
        melee_x_tap_timer=melee_x_tap_timer,
        melee_y_tap_timer=melee_y_tap_timer,
        melee_dash_direction=melee_dash_direction,
    )

    player.upkey = _button(joystick, 8)
    player.downkey = _button(joystick, 9)
    player.leftkey = _button(joystick, 10)
    player.rightkey = _button(joystick, 11)


def _merge_keyboard(player, keys):
    x_axis = 0
    y_axis = 0

    if _pressed(keys, pygame.K_a) or _pressed(keys, pygame.K_LEFT):
        x_axis -= 1
    if _pressed(keys, pygame.K_d) or _pressed(keys, pygame.K_RIGHT):
        x_axis += 1
    if _pressed(keys, pygame.K_w) or _pressed(keys, pygame.K_UP):
        y_axis -= 1
    if _pressed(keys, pygame.K_s) or _pressed(keys, pygame.K_DOWN):
        y_axis += 1

    player.main_stick = [
        _combine_axis(x_axis, player.main_stick[0]),
        _combine_axis(y_axis, player.main_stick[1]),
    ]

    player.akey = player.akey or _pressed(keys, pygame.K_j) or _pressed(keys, pygame.K_SPACE)
    player.specialkey = player.specialkey or _pressed(keys, pygame.K_u)
    player.jumpkey = (
        player.jumpkey
        or _pressed(keys, pygame.K_k)
        or _pressed(keys, pygame.K_w)
        or _pressed(keys, pygame.K_UP)
    )
    player.grabkey = player.grabkey or _pressed(keys, pygame.K_i)
    player.melee_attack_pressed = bool(
        getattr(player, "melee_attack_pressed", False)
        or _pressed(keys, pygame.K_j)
        or _pressed(keys, pygame.K_SPACE)
    )
    player.melee_special_pressed = bool(
        getattr(player, "melee_special_pressed", False) or _pressed(keys, pygame.K_u)
    )
    player.melee_grab_pressed = bool(
        getattr(player, "melee_grab_pressed", False) or _pressed(keys, pygame.K_i)
    )
    _merge_shield_state(
        player,
        left_shield=_pressed(keys, pygame.K_l),
        right_shield=_pressed(keys, pygame.K_LSHIFT),
        left_digital=_pressed(keys, pygame.K_l),
        right_digital=_pressed(keys, pygame.K_LSHIFT),
    )
    player.melee_shield_held = bool(
        getattr(player, "melee_shield_held", False) or player.blockkey
    )
    player.melee_shield_pressed = bool(
        getattr(player, "melee_shield_pressed", False) or player.block_pressed
    )
    player.melee_air_dodge_pressed = bool(
        getattr(player, "melee_air_dodge_pressed", False)
        or player.l_trigger_digital_pressed
        or player.r_trigger_digital_pressed
    )
    player.menukey = player.menukey or _pressed(keys, pygame.K_p) or _pressed(keys, pygame.K_RETURN)

    player.upkey = player.upkey or _pressed(keys, pygame.K_w) or _pressed(keys, pygame.K_UP)
    player.downkey = player.downkey or _pressed(keys, pygame.K_s) or _pressed(keys, pygame.K_DOWN)
    player.leftkey = player.leftkey or _pressed(keys, pygame.K_a) or _pressed(keys, pygame.K_LEFT)
    player.rightkey = player.rightkey or _pressed(keys, pygame.K_d) or _pressed(keys, pygame.K_RIGHT)

    if not player.jumpkey:
        if getattr(player, "jumpSquat", False):
            player.jump_released_during_squat = True
        else:
            player.canJump = True
    if not player.grabkey:
        player.releasePause = True
    if not player.menukey:
        player.menu = True


def gather_inputs(player, joystick=None, *, keys=None, keyboard=False):
    if joystick is None:
        if keyboard:
            _gather_keyboard(player, keys)
        else:
            _gather_idle(player)
        return

    release_state = {
        "canJump": getattr(player, "canJump", None),
        "jump_released_during_squat": getattr(
            player, "jump_released_during_squat", None
        ),
        "releasePause": getattr(player, "releasePause", None),
        "canBlock": getattr(player, "canBlock", None),
        "menu": getattr(player, "menu", None),
    }

    _gather_joystick(player, joystick)
    joystick_buttons = {
        "jumpkey": player.jumpkey,
        "grabkey": player.grabkey,
        "blockkey": player.blockkey,
        "menukey": player.menukey,
    }

    if keyboard:
        _merge_keyboard(player, keys)

        if player.jumpkey and not joystick_buttons["jumpkey"] and release_state["canJump"] is not None:
            player.canJump = release_state["canJump"]
            if release_state["jump_released_during_squat"] is not None:
                player.jump_released_during_squat = release_state[
                    "jump_released_during_squat"
                ]
        if player.grabkey and not joystick_buttons["grabkey"] and release_state["releasePause"] is not None:
            player.releasePause = release_state["releasePause"]
        if player.blockkey and not joystick_buttons["blockkey"] and release_state["canBlock"] is not None:
            player.canBlock = release_state["canBlock"]
        if player.menukey and not joystick_buttons["menukey"] and release_state["menu"] is not None:
            player.menu = release_state["menu"]
