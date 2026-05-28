use mole_core::PlayerInput;
use mole_input::{
    map_gamecube_pad_to_player_input, map_gamecube_pad_to_player_input_with_config,
    GameCubeButtonState, GameCubePadStatus, InputMappingConfig, InputOrigin,
};

#[test]
fn neutral_gamecube_pad_maps_to_neutral_player_input() {
    let input = map_gamecube_pad_to_player_input(GameCubePadStatus::neutral());

    assert_eq!(input, PlayerInput::neutral());
}

#[test]
fn gamecube_main_stick_maps_native_byte_extremes_without_float_normalization() {
    let left_down = map_gamecube_pad_to_player_input(GameCubePadStatus {
        stick_x: 0,
        stick_y: 0,
        ..GameCubePadStatus::neutral()
    });
    let right_up = map_gamecube_pad_to_player_input(GameCubePadStatus {
        stick_x: 255,
        stick_y: 255,
        ..GameCubePadStatus::neutral()
    });

    assert_eq!(left_down.stick_x(), -128);
    assert_eq!(left_down.stick_y(), -128);
    assert_eq!(right_up.stick_x(), 127);
    assert_eq!(right_up.stick_y(), 127);
}

#[test]
fn gamecube_mapping_preserves_c_stick_dpad_triggers_and_buttons() {
    let input = map_gamecube_pad_to_player_input(GameCubePadStatus {
        c_stick_x: 255,
        c_stick_y: 0,
        left_trigger: 17,
        right_trigger: 250,
        buttons: GameCubeButtonState::empty()
            .with_a(true)
            .with_b(true)
            .with_x(true)
            .with_y(true)
            .with_z(true)
            .with_l(true)
            .with_r(true)
            .with_start(true)
            .with_dpad_up(true)
            .with_dpad_left(true),
        ..GameCubePadStatus::neutral()
    });

    assert_eq!(input.c_stick_x(), 127);
    assert_eq!(input.c_stick_y(), -128);
    assert_eq!(input.left_trigger_analog(), 17);
    assert_eq!(input.right_trigger_analog(), 250);
    assert!(input.attack());
    assert!(input.special());
    assert!(input.jump_primary());
    assert!(input.jump_secondary());
    assert!(input.grab());
    assert!(input.left_trigger_digital());
    assert!(input.right_trigger_digital());
    assert!(input.start());
    assert!(input.dpad_up());
    assert!(input.dpad_left());
    assert!(!input.dpad_down());
    assert!(!input.dpad_right());
}

#[test]
fn input_origin_recenters_sticks_and_triggers_before_mapping() {
    let origin = InputOrigin::from_stable_sample(GameCubePadStatus {
        stick_x: 130,
        stick_y: 126,
        c_stick_x: 129,
        c_stick_y: 127,
        left_trigger: 8,
        right_trigger: 12,
        ..GameCubePadStatus::neutral()
    });

    let input = origin.map_gamecube_pad(GameCubePadStatus {
        stick_x: 130,
        stick_y: 126,
        c_stick_x: 129,
        c_stick_y: 127,
        left_trigger: 8,
        right_trigger: 20,
        ..GameCubePadStatus::neutral()
    });

    assert_eq!(input.stick_x(), 0);
    assert_eq!(input.stick_y(), 0);
    assert_eq!(input.c_stick_x(), 0);
    assert_eq!(input.c_stick_y(), 0);
    assert_eq!(input.left_trigger_analog(), 0);
    assert_eq!(input.right_trigger_analog(), 8);
}

#[test]
fn trigger_deadzone_maps_low_analog_values_to_zero_pressure() {
    let config = InputMappingConfig {
        trigger_deadzone: 8,
    };
    let input = map_gamecube_pad_to_player_input_with_config(
        GameCubePadStatus {
            left_trigger: 7,
            right_trigger: 8,
            ..GameCubePadStatus::neutral()
        },
        config,
    );

    assert_eq!(input.left_trigger_analog(), 0);
    assert_eq!(input.right_trigger_analog(), 0);
}

#[test]
fn trigger_deadzone_remaps_above_deadzone_to_full_analog_range() {
    let config = InputMappingConfig {
        trigger_deadzone: 8,
    };
    let input = map_gamecube_pad_to_player_input_with_config(
        GameCubePadStatus {
            left_trigger: 132,
            right_trigger: 255,
            ..GameCubePadStatus::neutral()
        },
        config,
    );

    assert_eq!(input.left_trigger_analog(), 128);
    assert_eq!(input.right_trigger_analog(), 255);
}

#[test]
fn trigger_deadzone_keeps_left_right_and_digital_bottom_out_independent() {
    let config = InputMappingConfig {
        trigger_deadzone: 8,
    };
    let input = map_gamecube_pad_to_player_input_with_config(
        GameCubePadStatus {
            left_trigger: 7,
            right_trigger: 132,
            buttons: GameCubeButtonState::empty().with_l(true),
            ..GameCubePadStatus::neutral()
        },
        config,
    );

    assert_eq!(input.left_trigger_analog(), 0);
    assert_eq!(input.right_trigger_analog(), 128);
    assert!(input.left_trigger_digital());
    assert!(!input.right_trigger_digital());
}
