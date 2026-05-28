use mole_core::{MeleeInputThresholds, MeleeInputTimers};
use mole_input::{GameCubeInputMapper, GameCubePadStatus};

#[test]
fn gamecube_input_mapper_carries_ucf_dashback_fact_without_state_command() {
    let mut mapper = GameCubeInputMapper::default();

    mapper.map_gamecube_pad(GameCubePadStatus::neutral());
    let tilt = mapper.map_gamecube_pad(GameCubePadStatus {
        stick_x: 168,
        ..GameCubePadStatus::neutral()
    });
    let dash = mapper.map_gamecube_pad(GameCubePadStatus {
        stick_x: 255,
        ..GameCubePadStatus::neutral()
    });

    assert!(!tilt.ucf_x_tilt_intent());
    assert!(dash.ucf_x_tilt_intent());
    assert!(!dash.attack());
    assert!(!dash.special());
    assert!(!dash.explicit_shield());

    let snapshot = dash.melee_snapshot(tilt, MeleeInputTimers::expired());
    let facts = snapshot.facts(MeleeInputThresholds::default());

    assert_eq!(facts.ucf_dashback_direction, 1);
    assert_eq!(facts.dash_direction, 1);
}

#[test]
fn gamecube_input_mapper_carries_ucf_shield_drop_fact_without_state_command() {
    let mut mapper = GameCubeInputMapper::default();

    mapper.map_gamecube_pad(GameCubePadStatus::neutral());
    let shallow_down = mapper.map_gamecube_pad(GameCubePadStatus {
        stick_y: 104,
        ..GameCubePadStatus::neutral()
    });
    let shield_drop = mapper.map_gamecube_pad(GameCubePadStatus {
        stick_y: 48,
        ..GameCubePadStatus::neutral()
    });

    assert!(!shallow_down.ucf_shield_drop_tilt_intent());
    assert!(shield_drop.ucf_shield_drop_tilt_intent());
    assert!(!shield_drop.attack());
    assert!(!shield_drop.special());
    assert!(!shield_drop.explicit_shield());

    let snapshot = shield_drop.melee_snapshot(shallow_down, MeleeInputTimers::expired());
    let facts = snapshot.facts(MeleeInputThresholds::default());

    assert!(facts.ucf_shield_drop);
}
