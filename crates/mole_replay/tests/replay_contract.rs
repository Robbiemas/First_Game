use mole_core::{step_world, Frame, PlayerInput, World};
use mole_replay::{ReplayFrame, ReplayLog};

#[test]
fn replay_log_validates_matching_checksums() {
    let initial = World::for_two_players();
    let mut world = initial.clone();
    let inputs = [
        PlayerInput::neutral().with_left_stick(127, 0),
        PlayerInput::neutral(),
    ];
    step_world(&mut world, Frame(0), &inputs);
    let mut log = ReplayLog::new(initial);

    log.push(ReplayFrame {
        frame: Frame(0),
        inputs,
        checksum: world.checksum(),
    });

    assert!(log.validate().is_ok());
}

#[test]
fn replay_log_detects_checksum_mismatch() {
    let mut log = ReplayLog::new(World::for_two_players());

    log.push(ReplayFrame {
        frame: Frame(0),
        inputs: [PlayerInput::neutral(), PlayerInput::neutral()],
        checksum: 1,
    });

    assert!(log.validate().is_err());
}
