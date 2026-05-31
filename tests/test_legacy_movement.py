from Characters import Character
from ChooseAction import resolve_action_state
from GatherInputs import gather_inputs


class PlayerStub:
    pass


class FakeJoystick:
    def __init__(self, name="Native WUP-028 Port 1", axes=None, buttons=None, melee=None):
        self.name = name
        self.axes = axes or {}
        self.buttons = buttons or {}
        self.melee = melee or {}

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

    def get_melee_fact(self, name, default=None):
        return self.melee.get(name, default)


def make_grounded_player():
    players = []
    player = Character(0, 0, players)
    player.dolphinmole()
    player.grounded = 1
    player.set_state("standing")
    player.actionable = True
    player.canDash = True
    player.canJump = True
    player.canBlock = True
    return player


def tick_stick(player, x, y=0):
    player.main_stick = [x, y]
    resolve_action_state(player)


def set_shield_inputs(
    player,
    *,
    left=False,
    right=False,
    left_pressed=False,
    right_pressed=False,
    left_digital=False,
    right_digital=False,
    left_digital_pressed=False,
    right_digital_pressed=False,
):
    player.l_shieldkey = left
    player.r_shieldkey = right
    player.l_shield_pressed = left_pressed
    player.r_shield_pressed = right_pressed
    player.l_trigger_digital = left_digital
    player.r_trigger_digital = right_digital
    player.l_trigger_digital_pressed = left_digital_pressed
    player.r_trigger_digital_pressed = right_digital_pressed
    player.blockkey = left or right
    player.block_pressed = left_pressed or right_pressed


def simulated_jump_height(initial_velocity, gravity_step):
    y = 0
    velocity = initial_velocity
    highest_point = 0

    for _ in range(240):
        velocity += gravity_step
        y += velocity
        highest_point = min(highest_point, y)
        if velocity >= 0:
            break

    return -highest_point


def assert_jump_velocity_matches_height(player, velocity, height):
    gravity_step = player.gWeight * player.multiplier
    expected_height = height * player.multiplier
    actual_height = simulated_jump_height(velocity, gravity_step)

    assert abs(actual_height - expected_height) < 0.05


def settle_walk_velocity(stick_x, frames=30):
    player = make_grounded_player()
    for _ in range(frames):
        tick_stick(player, stick_x)
    return player


def test_native_gamecube_micro_walk_input_survives_deadzone():
    player = PlayerStub()
    joystick = FakeJoystick(axes={0: 0.11})

    gather_inputs(player, joystick)

    assert player.main_stick[0] == 0.11


def test_dash_tap_refreshes_when_walk_input_crosses_dash_threshold():
    player = make_grounded_player()

    for _ in range(5):
        tick_stick(player, 0.2)

    tick_stick(player, 1.0)

    assert player.state == "dashing"
    assert player.isRight is True
    assert player.xVelocity > 0


def test_soft_fast_walk_input_does_not_dash_from_standing():
    player = make_grounded_player()

    tick_stick(player, 0.7)

    assert player.state == "walking"
    assert player.isRight is True


def test_soft_fast_walk_input_does_not_dash_after_held_walk():
    player = make_grounded_player()

    for _ in range(5):
        tick_stick(player, 0.2)

    tick_stick(player, 0.7)

    assert player.state == "walking"
    assert player.isRight is True


def test_walk_velocity_scales_analog_between_nearby_stick_values():
    soft = settle_walk_velocity(0.20)
    firmer = settle_walk_velocity(0.30)

    assert soft.state == "walking"
    assert firmer.state == "walking"
    assert firmer.xVelocity > soft.xVelocity * 1.35


def test_walk_velocity_settles_to_stick_scaled_target():
    player = settle_walk_velocity(0.50)
    target = 0.50 * player.walkSpeed * player.multiplier

    assert player.state == "walking"
    assert abs(player.xVelocity - target) < 0.05


def test_walk_velocity_mirrors_left_and_right():
    right = settle_walk_velocity(0.45)
    left = settle_walk_velocity(-0.45)

    assert right.state == "walking"
    assert left.state == "walking"
    assert abs(right.xVelocity + left.xVelocity) < 0.05


def test_full_run_approaches_captain_falcon_run_speed_without_oscillation():
    player = make_grounded_player()
    velocities = []

    for _ in range(25):
        tick_stick(player, 1.0)
        velocities.append(player.xVelocity)

    max_run_velocity = player.runSpeed * player.multiplier

    assert player.state == "running"
    assert max(abs(velocity) for velocity in velocities[-5:]) <= max_run_velocity + 0.01
    assert max(velocities[-5:]) - min(velocities[-5:]) < 0.05
    assert abs(player.xVelocity - max_run_velocity) < 0.05


def test_gravity_applies_additive_captain_falcon_acceleration():
    player = make_grounded_player()
    player.yVelocity = 0
    expected_step = player.gWeight * player.multiplier

    player.gravity(player.gWeight)

    assert abs(player.yVelocity - expected_step) < 0.001

    player.gravity(player.gWeight)

    assert abs(player.yVelocity - expected_step * 2) < 0.001


def test_held_down_before_falling_does_not_fast_fall_late():
    player = make_grounded_player()
    player.grounded = 0
    player.set_state("air")
    player.actionable = True
    player.canJump = False

    for _ in range(4):
        player.yVelocity = -4
        tick_stick(player, 0.0, 1.0)

    player.yVelocity = 1
    tick_stick(player, 0.0, 1.0)

    assert player.yVelocity == 1 + (player.gWeight * player.multiplier)


def test_fresh_down_tap_while_falling_consumes_fast_fall():
    player = make_grounded_player()
    player.grounded = 0
    player.set_state("air")
    player.actionable = True
    player.canJump = False
    player.yVelocity = 1

    tick_stick(player, 0.0, 0.0)
    tick_stick(player, 0.0, 1.0)

    assert player.yVelocity == player.fastFallSpeed
    assert player.yTapTimer == 0xFE


def test_slow_stick_travel_to_dash_threshold_does_not_dash():
    player = make_grounded_player()

    for x in (0.30, 0.50, 0.70, 0.85, 1.0):
        tick_stick(player, x)

    assert player.state == "walking"
    assert player.isRight is True


def test_held_walk_then_far_stick_does_not_dash_without_new_tap():
    player = make_grounded_player()

    for _ in range(4):
        tick_stick(player, 0.40)

    tick_stick(player, 1.0)

    assert player.state == "walking"
    assert player.isRight is True


def test_fast_stick_travel_to_dash_threshold_still_dashes():
    player = make_grounded_player()

    tick_stick(player, 0.40)
    tick_stick(player, 1.0)

    assert player.state == "dashing"
    assert player.isRight is True


def test_native_melee_dash_fact_drives_pygame_dash_when_legacy_window_would_expire():
    player = make_grounded_player()
    player.xTapDirection = 1
    player.xTapTimer = 2
    joystick = FakeJoystick(
        axes={0: 1.0},
        melee={
            "lstick_x": 127,
            "lstick_y": 0,
            "cstick_x": 0,
            "cstick_y": 0,
            "x_tap_timer": 2,
            "dash_direction": 1,
        },
    )

    gather_inputs(player, joystick)
    resolve_action_state(player)

    assert player.state == "dashing"
    assert player.isRight is True


def test_native_melee_dash_fact_blocks_legacy_dash_when_rust_timer_is_expired():
    player = make_grounded_player()
    player.xTapDirection = 1
    player.xTapTimer = 0
    joystick = FakeJoystick(
        axes={0: 1.0},
        melee={
            "lstick_x": 127,
            "lstick_y": 0,
            "cstick_x": 0,
            "cstick_y": 0,
            "x_tap_timer": 4,
            "dash_direction": 0,
        },
    )

    gather_inputs(player, joystick)
    resolve_action_state(player)

    assert player.state == "walking"
    assert player.isRight is True


def test_native_canonical_dashback_fact_allows_dash_dance_when_timer_misses():
    player = make_grounded_player()

    frames = [
        (1.0, {"x_tap_timer": 0, "dash_direction": 1}),
        (1.0, {"x_tap_timer": 1, "dash_direction": 1}),
        (-1.0, {"x_tap_timer": 0xFE, "dash_direction": -1}),
        (-1.0, {"x_tap_timer": 0xFE, "dash_direction": -1}),
    ]

    for x, melee in frames:
        gather_inputs(
            player,
            FakeJoystick(
                axes={0: x},
                melee={
                    "lstick_x": int(x * 127),
                    "lstick_y": 0,
                    "cstick_x": 0,
                    "cstick_y": 0,
                    **melee,
                },
            ),
        )
        resolve_action_state(player)

    assert player.state == "dashing"
    assert player.isRight is False
    assert player.xVelocity < 0


def test_dash_can_reverse_during_early_initial_dash():
    player = make_grounded_player()

    tick_stick(player, 1.0)
    tick_stick(player, 1.0)
    tick_stick(player, -1.0)

    assert player.state == "standing"
    assert player.actionable is True
    assert player.dashTurn is True
    assert player.smashTurn is True
    assert player.xVelocity == 0

    tick_stick(player, -1.0)

    assert player.state == "dashing"
    assert player.isRight is False
    assert player.xVelocity < 0


def test_dash_pivot_turns_around_when_opposite_dash_is_released_to_neutral():
    player = make_grounded_player()

    tick_stick(player, 1.0)
    tick_stick(player, 1.0)
    tick_stick(player, -1.0)
    tick_stick(player, 0.0)

    assert player.state == "standing"
    assert player.isRight is False
    assert player.xVelocity == 0


def test_soft_reverse_stick_during_dash_creates_moonwalk_brake_without_turnaround():
    player = make_grounded_player()

    tick_stick(player, 1.0)
    tick_stick(player, 1.0)
    start_velocity = player.xVelocity
    tick_stick(player, -0.7)

    assert player.state == "dashing"
    assert player.isRight is True
    assert 0 < player.xVelocity < start_velocity


def test_light_back_setup_then_full_back_moonwalks_instead_of_pivoting():
    player = make_grounded_player()

    tick_stick(player, 1.0)
    tick_stick(player, 1.0)
    tick_stick(player, -0.7, 0.6)
    tick_stick(player, -0.7, 0.6)

    velocity_before_full_back = player.xVelocity

    tick_stick(player, -1.0)
    tick_stick(player, -1.0)
    tick_stick(player, -1.0)

    assert player.state == "dashing"
    assert player.isRight is True
    assert player.xVelocity < velocity_before_full_back


def test_light_back_setup_then_full_back_moonwalks_without_y_axis_gate():
    player = make_grounded_player()

    tick_stick(player, 1.0)
    tick_stick(player, 1.0)
    tick_stick(player, -0.7, 0.0)
    tick_stick(player, -0.7, 0.0)

    velocity_before_full_back = player.xVelocity

    tick_stick(player, -1.0, 0.0)

    assert player.state == "dashing"
    assert player.isRight is True
    assert player.xVelocity < velocity_before_full_back


def test_light_back_setup_then_full_back_moonwalks_without_y_axis_gate_from_left_dash():
    player = make_grounded_player()

    tick_stick(player, -1.0)
    tick_stick(player, -1.0)
    tick_stick(player, 0.7, 0.0)
    tick_stick(player, 0.7, 0.0)

    velocity_before_full_back = player.xVelocity

    tick_stick(player, 1.0, 0.0)

    assert player.state == "dashing"
    assert player.isRight is False
    assert player.xVelocity > velocity_before_full_back


def test_light_back_setup_then_full_back_moonwalks_symmetrically_from_left_dash():
    player = make_grounded_player()

    tick_stick(player, -1.0)
    tick_stick(player, -1.0)
    tick_stick(player, 0.7, 0.6)
    tick_stick(player, 0.7, 0.6)

    velocity_before_full_back = player.xVelocity

    tick_stick(player, 1.0)
    tick_stick(player, 1.0)
    tick_stick(player, 1.0)

    assert player.state == "dashing"
    assert player.isRight is False
    assert player.xVelocity > velocity_before_full_back


def test_high_down_away_back_input_moonwalks_without_pivoting_right_dash():
    player = make_grounded_player()

    tick_stick(player, 1.0)
    tick_stick(player, 1.0)
    tick_stick(player, -0.70, 0.6)

    velocity_before_back_input = player.xVelocity

    tick_stick(player, -0.70, 0.5)
    tick_stick(player, -1.0, 0.0)

    assert player.state == "dashing"
    assert player.isRight is True
    assert player.xVelocity < velocity_before_back_input


def test_high_down_away_back_input_moonwalks_without_pivoting_left_dash():
    player = make_grounded_player()

    tick_stick(player, -1.0)
    tick_stick(player, -1.0)
    tick_stick(player, 0.70, 0.6)

    velocity_before_back_input = player.xVelocity

    tick_stick(player, 0.70, 0.5)
    tick_stick(player, 1.0, 0.0)

    assert player.state == "dashing"
    assert player.isRight is False
    assert player.xVelocity > velocity_before_back_input


def test_holding_back_after_right_dash_moonwalk_settles_into_walk_not_run():
    player = make_grounded_player()

    tick_stick(player, 1.0)
    tick_stick(player, 1.0)
    tick_stick(player, -0.70, 0.6)
    tick_stick(player, -0.70, 0.5)

    held_states = []
    for _ in range(35):
        tick_stick(player, -1.0, 0.0)
        held_states.append(player.state)

    assert "running" not in held_states
    assert "dashing" not in held_states[player.dashFrames:]
    assert player.state == "walking"
    assert player.isRight is False
    assert abs(player.xVelocity) <= player.walkSpeed * player.multiplier


def test_holding_back_after_left_dash_moonwalk_settles_into_walk_not_run():
    player = make_grounded_player()

    tick_stick(player, -1.0)
    tick_stick(player, -1.0)
    tick_stick(player, 0.70, 0.6)
    tick_stick(player, 0.70, 0.5)

    held_states = []
    for _ in range(35):
        tick_stick(player, 1.0, 0.0)
        held_states.append(player.state)

    assert "running" not in held_states
    assert "dashing" not in held_states[player.dashFrames:]
    assert player.state == "walking"
    assert player.isRight is True
    assert abs(player.xVelocity) <= player.walkSpeed * player.multiplier


def test_dash_to_walk_transition_does_not_apply_walk_physics_until_next_tick():
    player = make_grounded_player()

    tick_stick(player, 1.0)
    tick_stick(player, 1.0)
    tick_stick(player, -0.7)
    tick_stick(player, -0.7)

    last_dash_velocity = player.xVelocity
    for _ in range(25):
        if player.state == "dashing":
            last_dash_velocity = player.xVelocity
        tick_stick(player, -1.0)
        if player.state == "walking":
            break

    expected_dash_only_velocity = last_dash_velocity - (
        (player.dashAccelBase + player.dashAccelAdd) * player.multiplier
    )

    assert player.state == "walking"
    assert abs(player.xVelocity - expected_dash_only_velocity) < 0.001


def test_run_turn_entry_does_not_apply_opposite_run_acceleration_same_tick():
    player = make_grounded_player()

    for _ in range(25):
        tick_stick(player, 1.0)

    assert player.state == "running"
    run_velocity = player.xVelocity

    tick_stick(player, -1.0)

    expected_turn_entry_velocity = run_velocity * (1 - player.traction)
    assert player.state == "runTurn"
    assert abs(player.xVelocity - expected_turn_entry_velocity) < 0.001


def test_walk_soft_opposite_stick_exits_to_wait_without_flipping_facing():
    player = make_grounded_player()

    tick_stick(player, 0.40)
    assert player.state == "walking"
    assert player.isRight is True

    tick_stick(player, -0.40)

    assert player.state == "standing"
    assert player.isRight is True
    assert player.xVelocity == 0


def test_run_turn_keeps_old_facing_until_velocity_crosses_zero():
    player = make_grounded_player()

    for _ in range(25):
        tick_stick(player, 1.0)

    tick_stick(player, -1.0)
    assert player.state == "runTurn"
    assert player.isRight is True
    entry_velocity = player.xVelocity

    tick_stick(player, -1.0)

    assert player.state == "runTurn"
    assert player.isRight is True
    assert 0 < player.xVelocity < entry_velocity


def test_run_turn_allows_jump_cancel_before_long_turn_window():
    player = make_grounded_player()

    for _ in range(25):
        tick_stick(player, 1.0)

    tick_stick(player, -1.0)
    assert player.state == "runTurn"

    player.main_stick = [-1.0, 0.0]
    player.jumpkey = True
    resolve_action_state(player)

    assert player.state == "jumpSquat"


def test_neutral_center_reset_allows_fresh_opposite_dash_pivot():
    player = make_grounded_player()

    tick_stick(player, 1.0)
    tick_stick(player, 1.0)
    tick_stick(player, 0.0, 0.0)
    tick_stick(player, -1.0, 0.0)

    assert player.state == "standing"
    assert player.dashTurn is True
    assert player.xVelocity == 0


def test_neutral_center_reset_allows_fresh_opposite_dash_pivot_from_left_dash():
    player = make_grounded_player()

    tick_stick(player, -1.0)
    tick_stick(player, -1.0)
    tick_stick(player, 0.0, 0.0)
    tick_stick(player, 1.0, 0.0)

    assert player.state == "standing"
    assert player.dashTurn is True
    assert player.xVelocity == 0


def test_shield_press_on_final_jumpsquat_frame_does_not_airdodge_during_jumpsquat():
    player = make_grounded_player()

    set_shield_inputs(player, left=True, left_pressed=True)
    resolve_action_state(player)

    player.jumpkey = True
    set_shield_inputs(player, left=True)
    resolve_action_state(player)

    set_shield_inputs(player, left=True)
    resolve_action_state(player)

    set_shield_inputs(player, left=True)
    resolve_action_state(player)

    player.main_stick = [1.0, 1.0]
    set_shield_inputs(
        player,
        left=True,
        right=True,
        right_pressed=True,
        right_digital=True,
        right_digital_pressed=True,
    )
    resolve_action_state(player)

    assert player.state == "jumpSquat"
    assert player.grounded == 0
    assert player.yVelocity == 0

    set_shield_inputs(player, left=True, right=True)
    resolve_action_state(player)

    assert player.state == "air"
    assert player.yVelocity == player.jumpHeight + (player.gWeight * player.multiplier)


def test_fresh_shield_press_first_airborne_frame_after_jumpsquat_airdodges():
    player = make_grounded_player()

    set_shield_inputs(player, left=True, left_pressed=True)
    resolve_action_state(player)

    player.jumpkey = True
    set_shield_inputs(player, left=True)
    resolve_action_state(player)

    set_shield_inputs(player, left=True)
    resolve_action_state(player)

    set_shield_inputs(player, left=True)
    resolve_action_state(player)

    set_shield_inputs(player, left=True)
    resolve_action_state(player)

    assert player.state == "jumpSquat"
    assert player.grounded == 0

    player.main_stick = [1.0, 1.0]
    set_shield_inputs(
        player,
        left=True,
        right=True,
        right_pressed=True,
        right_digital=True,
        right_digital_pressed=True,
    )
    resolve_action_state(player)

    assert player.state == "airDodge"
    assert player.dodgeCount == 1
    assert player.xVelocity > 0
    assert player.yVelocity > 0


def test_analog_trigger_press_first_airborne_frame_does_not_airdodge():
    player = make_grounded_player()

    player.jumpkey = True
    resolve_action_state(player)

    for _ in range(player.js):
        player.jumpkey = True
        resolve_action_state(player)

    assert player.state == "air"
    assert player.grounded == 0

    player.main_stick = [1.0, 1.0]
    set_shield_inputs(player, right=True, right_pressed=True)
    resolve_action_state(player)

    assert player.state == "air"
    assert player.airDodge is False


def test_rust_air_dodge_fact_overrides_legacy_shield_press_fallback():
    player = make_grounded_player()
    player.grounded = 0
    player.set_state("air")
    player.actionable = True
    player.canJump = False
    player.melee_air_dodge_pressed = False

    player.main_stick = [1.0, 1.0]
    set_shield_inputs(
        player,
        left=True,
        left_pressed=True,
        left_digital=True,
        left_digital_pressed=False,
    )
    resolve_action_state(player)

    assert player.state == "air"
    assert player.airDodge is False


def test_fresh_digital_trigger_airdodges_while_other_trigger_is_held():
    player = make_grounded_player()

    player.jumpkey = True
    resolve_action_state(player)

    for _ in range(player.js):
        player.jumpkey = True
        resolve_action_state(player)

    assert player.state == "air"
    assert player.grounded == 0

    set_shield_inputs(player, left=True)
    resolve_action_state(player)

    player.main_stick = [1.0, 1.0]
    set_shield_inputs(
        player,
        left=True,
        right=True,
        right_pressed=True,
        right_digital=True,
        right_digital_pressed=True,
    )
    resolve_action_state(player)

    assert player.state == "airDodge"
    assert player.dodgeCount == 1


def test_held_shield_does_not_reenter_blocking_each_frame():
    player = make_grounded_player()

    set_shield_inputs(player, left=True, left_pressed=True)
    resolve_action_state(player)

    assert player.state == "blocking"

    player.aniCount = 7
    set_shield_inputs(player, left=True)
    resolve_action_state(player)

    assert player.state == "blocking"
    assert player.aniCount == 7


def test_releasing_shield_enters_guard_off_before_standing():
    player = make_grounded_player()

    set_shield_inputs(player, left=True, left_pressed=True)
    resolve_action_state(player)

    assert player.state == "blocking"

    set_shield_inputs(player)
    resolve_action_state(player)

    assert player.state == "guardOff"
    assert player.blocking is False
    assert player.shielding is False
    assert player.actionable is False

    for _ in range(player.guardOffFrames - 1):
        set_shield_inputs(player)
        resolve_action_state(player)
        assert player.state == "guardOff"

    set_shield_inputs(player)
    resolve_action_state(player)

    assert player.state == "standing"
    assert player.actionable is True
    assert player.canDash is True


def test_shield_release_lag_does_not_restart_shield_turn_when_stick_is_held():
    player = make_grounded_player()

    set_shield_inputs(player, left=True, left_pressed=True)
    resolve_action_state(player)

    player.main_stick = [-1.0, 0]
    set_shield_inputs(player)
    resolve_action_state(player)

    assert player.state == "guardOff"
    assert player.shieldTurn is False

    for _ in range(player.guardOffFrames):
        player.main_stick = [-1.0, 0]
        set_shield_inputs(player)
        resolve_action_state(player)

    assert player.state in {"standing", "turning"}
    assert player.blocking is False
    assert player.guardOff is False


def test_shield_release_frame_enters_guard_off_before_jump_check():
    player = make_grounded_player()

    set_shield_inputs(player, left=True, left_pressed=True)
    resolve_action_state(player)

    player.jumpkey = True
    set_shield_inputs(player)
    resolve_action_state(player)

    assert player.state == "guardOff"

    player.jumpkey = True
    set_shield_inputs(player)
    resolve_action_state(player)

    assert player.state == "jumpSquat"


def test_full_hop_out_of_shield_when_jump_is_held():
    player = make_grounded_player()

    set_shield_inputs(player, left=True, left_pressed=True)
    resolve_action_state(player)

    player.jumpkey = True
    set_shield_inputs(player, left=True)
    resolve_action_state(player)

    assert player.state == "jumpSquat"

    for _ in range(player.js):
        player.jumpkey = True
        set_shield_inputs(player, left=True)
        resolve_action_state(player)

    assert player.state == "air"
    assert player.yVelocity == player.jumpHeight + (player.gWeight * player.multiplier)


def test_jumpsquat_cannot_stick_if_stage_collision_regrounds_before_air_tick():
    player = make_grounded_player()

    player.jumpkey = True
    resolve_action_state(player)

    assert player.state == "jumpSquat"

    for _ in range(player.js + 4):
        player.jumpkey = True
        resolve_action_state(player)
        if player.state == "jumpSquat":
            player.grounded = True

    assert player.state == "air"
    assert not player.grounded
    assert player.yVelocity != 0


def test_short_hop_out_of_shield_when_jump_is_released_during_jumpsquat():
    player = make_grounded_player()

    set_shield_inputs(player, left=True, left_pressed=True)
    resolve_action_state(player)

    player.jumpkey = True
    set_shield_inputs(player, left=True)
    resolve_action_state(player)

    assert player.state == "jumpSquat"

    for _ in range(player.js):
        player.jumpkey = False
        set_shield_inputs(player, left=True)
        resolve_action_state(player)

    assert player.state == "air"
    assert player.yVelocity == player.shortHop + (player.gWeight * player.multiplier)


def test_jump_release_during_shield_jumpsquat_does_not_rearm_jump_startup():
    player = make_grounded_player()

    gather_inputs(player, FakeJoystick(buttons={5: True}))
    resolve_action_state(player)

    gather_inputs(player, FakeJoystick(buttons={2: True, 5: True}))
    resolve_action_state(player)

    assert player.state == "jumpSquat"
    assert player.canJump is False

    gather_inputs(player, FakeJoystick(buttons={5: True}))
    resolve_action_state(player)

    assert player.state == "jumpSquat"
    assert player.canJump is False
    assert player.jump_released_during_squat is True


def test_jump_repress_during_shield_jumpsquat_does_not_restart_jumpsquat():
    player = make_grounded_player()

    set_shield_inputs(player, left=True, left_pressed=True)
    resolve_action_state(player)

    player.jumpkey = True
    set_shield_inputs(player, left=True)
    resolve_action_state(player)

    player.jumpkey = False
    player.canJump = True
    set_shield_inputs(player, left=True)
    resolve_action_state(player)

    assert player.state == "jumpSquat"
    assert player.jsCount == 1

    player.jumpkey = True
    set_shield_inputs(player, left=True)
    resolve_action_state(player)

    assert player.state == "jumpSquat"
    assert player.jsCount == 2


def test_soft_opposite_stick_from_standing_starts_tilt_turn_not_dash_turn():
    player = make_grounded_player()

    tick_stick(player, -0.4)

    assert player.state == "turning"
    assert player.tiltTurn is True
    assert player.smashTurn is False
    assert player.canDash is False


def test_dolphinmole_uses_captain_falcon_timing_profile():
    player = make_grounded_player()
    frame_data = getattr(player, "frameData", {})

    assert player.weight == 104
    assert player.js == 4
    assert player.dashFrames == 15
    assert getattr(player, "landingLagFrames", None) == 4
    assert getattr(player, "airDodgeFrames", None) == 49
    assert frame_data.get("jab1") == {
        "first_active": 3,
        "last_active": 5,
        "total": 21,
        "iasa": 16,
    }


def test_dolphinmole_jump_velocities_match_captain_falcon_height_profile():
    player = make_grounded_player()

    assert_jump_velocity_matches_height(player, player.jumpHeight, player.fullHopHeight)
    assert_jump_velocity_matches_height(player, player.shortHop, player.shortHopHeight)
    assert_jump_velocity_matches_height(player, player.airJumpHeight, player.doubleJumpHeight)


def test_empty_landing_uses_captain_falcon_four_frame_lag():
    player = make_grounded_player()
    player.set_state("air")

    for _ in range(4):
        resolve_action_state(player)

    assert player.state == "standing"
    assert player.actionable is True
    assert player.landLagCount == 0


def test_air_dodge_uses_source_action_phase_then_post_movement_hold():
    player = make_grounded_player()
    player.set_state("airDodge")
    player.main_stick = [1, 0]

    for _ in range(player.airDodgeActionFrames):
        player.air_dodge()

    assert player.state == "airDodge"
    assert player.dodgeCount == player.airDodgeActionFrames
    assert player.xVelocity == 0
    assert player.yVelocity == 0

    for _ in range(player.airDodgeFrames - player.airDodgeActionFrames - 1):
        player.air_dodge()
        assert player.state == "airDodge"
        assert player.xVelocity == 0
        assert player.yVelocity == 0

    player.air_dodge()
    assert player.state == "fallSpecial"
    assert player.dodgeCount == 0


def test_air_dodge_stick_inside_source_deadzone_has_no_self_velocity():
    player = make_grounded_player()
    player.set_state("airDodge")
    source_deadzone = 20 / 127
    player.main_stick = [source_deadzone - 0.01, -(source_deadzone - 0.01)]

    player.air_dodge()

    assert player.xVelocity == 0
    assert player.yVelocity == 0


def test_air_dodge_self_velocity_decays_by_source_multiplier():
    player = make_grounded_player()
    player.set_state("airDodge")
    player.main_stick = [1, 0]

    player.air_dodge()
    first_velocity = player.xVelocity
    player.air_dodge()

    assert first_velocity > 0
    assert abs(player.xVelocity - (first_velocity * 0.90)) < 0.001
    assert player.yVelocity == 0


def test_air_dodge_action_phase_ends_in_static_escape_air_hold():
    player = make_grounded_player()
    player.set_state("airDodge")
    player.main_stick = [1, 0]

    for _ in range(15):
        player.air_dodge()

    assert player.state == "airDodge"
    assert player.dodgeCount == player.airDodgeActionFrames
    assert player.xVelocity == 0
    assert player.yVelocity == 0


def test_resolver_keeps_escape_air_static_until_animation_enters_fall_special():
    player = make_grounded_player()
    player.grounded = 0
    player.set_state("airDodge")
    player.actionable = False
    player.main_stick = [1, 0]

    for _ in range(player.airDodgeActionFrames):
        resolve_action_state(player)

    assert player.state == "airDodge"
    assert player.xVelocity == 0
    assert player.yVelocity == 0

    for _ in range(player.airDodgeFrames - player.airDodgeActionFrames - 1):
        resolve_action_state(player)
        assert player.state == "airDodge"
        assert player.xVelocity == 0
        assert player.yVelocity == 0

    resolve_action_state(player)

    assert player.state == "fallSpecial"
    assert player.dodgeCount == 0

    resolve_action_state(player)

    assert player.state == "fallSpecial"
    assert player.yVelocity > 0


def test_air_dodge_landing_uses_landing_fall_special_and_preserves_slide():
    player = make_grounded_player()
    player.set_state("airDodge")
    player.xVelocity = 12
    player.yVelocity = -5

    resolve_action_state(player)

    assert player.state == "landingFallSpecial"
    assert player.yVelocity == 0
    assert 0 < player.xVelocity < 12

    for _ in range(player.landingFallSpecialFrames - 1):
        resolve_action_state(player)

    assert player.state == "standing"
    assert player.actionable is True
    assert player.landLagCount == 0


def test_air_dodge_landing_cannot_enter_shield_from_held_trigger():
    player = make_grounded_player()
    player.set_state("airDodge")
    player.xVelocity = -12
    player.yVelocity = 5
    set_shield_inputs(player, right=True, right_digital=True)

    resolve_action_state(player)

    assert player.state == "landingFallSpecial"
    assert player.blocking is False
    assert player.actionable is False
    assert player.xVelocity < 0

    for _ in range(player.landingFallSpecialFrames - 2):
        set_shield_inputs(player, right=True, right_digital=True)
        resolve_action_state(player)
        assert player.state == "landingFallSpecial"
        assert player.blocking is False

    set_shield_inputs(player, right=True, right_digital=True)
    resolve_action_state(player)

    assert player.state == "standing"
    assert player.blocking is False
    assert player.actionable is True

    set_shield_inputs(player)
    resolve_action_state(player)

    assert player.state == "standing"
    assert player.blocking is False
    assert player.actionable is True


def test_landing_fall_special_slide_off_enters_air_instead_of_frozen_slide():
    player = make_grounded_player()
    player.grounded = 0
    player.set_state("landingFallSpecial")
    player.actionable = False
    player.canJump = False
    player.canDash = False
    player.landLagCount = 4
    player.xVelocity = 9
    player.yVelocity = 0

    resolve_action_state(player)

    assert player.state == "air"
    assert player.actionable is True
    assert player.landLagCount == 0
    assert player.xVelocity == 9
    assert player.yVelocity == 0

    resolve_action_state(player)

    assert player.state == "air"
    assert player.yVelocity == player.gWeight * player.multiplier


def test_empty_landing_slide_off_uses_same_fall_transition():
    player = make_grounded_player()
    player.grounded = 0
    player.set_state("landingLag")
    player.actionable = False
    player.landLagCount = 2
    player.xVelocity = -6
    player.yVelocity = 0

    resolve_action_state(player)

    assert player.state == "air"
    assert player.actionable is True
    assert player.landLagCount == 0
    assert player.xVelocity == -6
    assert player.yVelocity == 0
