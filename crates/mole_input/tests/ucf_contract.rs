use mole_core::{
    MeleeCommonData, MeleeInputThresholds, MeleeInputTimers, PlayerInput,
    UCF_DASHBACK_AMENDMENT_BIT,
};
use mole_input::{GameCubeInputMapper, GameCubePadStatus, InputMappingConfig};

#[test]
fn gamecube_input_mapper_applies_ucf_cardinal_snap_before_engine_input() {
    let mut mapper = GameCubeInputMapper::default();

    mapper.map_gamecube_pad(GameCubePadStatus::neutral());
    let snapped = mapper.map_gamecube_pad(GameCubePadStatus {
        stick_x: 208,
        stick_y: 133,
        ..GameCubePadStatus::neutral()
    });

    assert_eq!(snapped.stick_x(), 127);
    assert_eq!(snapped.stick_y(), 0);
    assert!(!snapped.attack());
    assert!(!snapped.special());
    assert!(!snapped.explicit_shield());

    let snapshot = snapped.melee_snapshot(PlayerInput::neutral(), MeleeInputTimers::expired());
    let facts = snapshot.facts(MeleeInputThresholds::default());

    assert_eq!(facts.dash_direction, 1);
}

#[test]
fn gamecube_input_mapper_can_disable_ucf_preprocessing() {
    let mut mapper = GameCubeInputMapper::new(InputMappingConfig {
        ucf_enabled: false,
        ..InputMappingConfig::default()
    });

    mapper.map_gamecube_pad(GameCubePadStatus::neutral());
    let unsnapped = mapper.map_gamecube_pad(GameCubePadStatus {
        stick_x: 208,
        stick_y: 133,
        ..GameCubePadStatus::neutral()
    });

    assert_eq!(unsnapped.stick_x(), 125);
    assert_eq!(unsnapped.stick_y(), 6);
}

#[test]
fn vanilla_gamecube_input_uses_hsd_scaled_cardinal_gate_without_ucf_snap() {
    let mut mapper = GameCubeInputMapper::new(InputMappingConfig {
        ucf_enabled: false,
        ..InputMappingConfig::default()
    });

    mapper.map_gamecube_pad(GameCubePadStatus::neutral());
    let right_gate = mapper.map_gamecube_pad(GameCubePadStatus {
        stick_x: 208,
        ..GameCubePadStatus::neutral()
    });
    let snapshot = right_gate.melee_snapshot(PlayerInput::neutral(), MeleeInputTimers::expired());
    let facts = snapshot.facts(MeleeInputThresholds::default());

    assert_eq!(right_gate.stick_x(), 127);
    assert_eq!(facts.dash_direction, 1);
    assert_eq!(right_gate.bits() & UCF_DASHBACK_AMENDMENT_BIT, 0);
}

#[test]
fn gamecube_input_mapper_marks_ucf_dashback_amendment_from_source_two_frame_x_delta() {
    let mut mapper = GameCubeInputMapper::default();

    mapper.map_gamecube_pad(GameCubePadStatus::neutral());
    mapper.map_gamecube_pad(GameCubePadStatus {
        stick_x: 88,
        ..GameCubePadStatus::neutral()
    });
    let amended_dashback = mapper.map_gamecube_pad(GameCubePadStatus {
        stick_x: 0,
        ..GameCubePadStatus::neutral()
    });

    assert_eq!(amended_dashback.stick_x(), -127);
    assert_ne!(
        amended_dashback.bits() & UCF_DASHBACK_AMENDMENT_BIT,
        0,
        "UCF 0.84 dashback is sourced from the raw two-frame x delta before the core sees the frame"
    );
}

#[test]
fn gamecube_input_mapper_does_not_mark_ucf_dashback_when_ucf_is_disabled() {
    let mut mapper = GameCubeInputMapper::new(InputMappingConfig {
        ucf_enabled: false,
        ..InputMappingConfig::default()
    });

    mapper.map_gamecube_pad(GameCubePadStatus::neutral());
    mapper.map_gamecube_pad(GameCubePadStatus {
        stick_x: 88,
        ..GameCubePadStatus::neutral()
    });
    let vanilla_dashback = mapper.map_gamecube_pad(GameCubePadStatus {
        stick_x: 0,
        ..GameCubePadStatus::neutral()
    });

    assert_eq!(vanilla_dashback.stick_x(), -127);
    assert_eq!(vanilla_dashback.bits() & UCF_DASHBACK_AMENDMENT_BIT, 0);
}

#[test]
fn gamecube_input_mapper_translates_ucf_shield_drop_after_source_two_frame_counter() {
    let mut mapper = GameCubeInputMapper::default();

    mapper.map_gamecube_pad(GameCubePadStatus::neutral());
    let shallow_down = mapper.map_gamecube_pad(GameCubePadStatus {
        stick_y: 112,
        ..GameCubePadStatus::neutral()
    });
    let shield_drop = mapper.map_gamecube_pad(GameCubePadStatus {
        stick_y: 0,
        buttons: mole_input::GameCubeButtonState::empty().with_l(true),
        ..GameCubePadStatus::neutral()
    });
    let shield_drop_ready = mapper.map_gamecube_pad(GameCubePadStatus {
        stick_y: 0,
        buttons: mole_input::GameCubeButtonState::empty().with_l(true),
        ..GameCubePadStatus::neutral()
    });
    let common = MeleeCommonData::provisional_mole();

    assert_eq!(shallow_down.stick_y(), -25);
    assert_eq!(shield_drop.stick_y(), -127);
    assert_eq!(shield_drop_ready.stick_y(), -127);
    assert!(shield_drop_ready.stick_y() <= common.escape_y);
    assert!(shield_drop_ready.ucf_shield_drop_amendment());
    assert!(!shield_drop.attack());
    assert!(!shield_drop.special());
    assert!(shield_drop.left_trigger_digital());

    let snapshot = shield_drop_ready.melee_snapshot(shallow_down, MeleeInputTimers::expired());
    let facts = snapshot.facts(MeleeInputThresholds::default());

    assert!(facts.main_stick_spot_dodge);
    assert_eq!(snapshot.y_tap_timer, 0);
}

#[test]
fn gamecube_input_mapper_marks_ucf_axe_method_shield_drop_without_rewriting_stick() {
    let mut mapper = GameCubeInputMapper::default();
    let shield = mole_input::GameCubeButtonState::empty().with_l(true);

    for (x, y) in [(95, 8), (91, 1), (89, -8), (89, -33), (85, -61)] {
        let _ = mapper.map_gamecube_pad(GameCubePadStatus {
            stick_x: (x + 128) as u8,
            stick_y: (y + 128) as u8,
            buttons: shield,
            ..GameCubePadStatus::neutral()
        });
    }

    let axe_drop = mapper.map_gamecube_pad(GameCubePadStatus {
        stick_x: (82 + 128) as u8,
        stick_y: (-90 + 128) as u8,
        buttons: shield,
        ..GameCubePadStatus::neutral()
    });

    assert_eq!(axe_drop.stick_x(), 84);
    assert_eq!(axe_drop.stick_y(), -94);
    assert!(
        axe_drop.ucf_shield_drop_amendment(),
        "UCF 0.84 Axe-method shield drop suppresses the main-stick spotdodge route on rim coords above the -0.8f Y cutoff after roll is disabled"
    );
}
