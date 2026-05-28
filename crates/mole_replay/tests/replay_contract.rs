use mole_core::{step_world, Frame, PlayerInput, World};
use mole_replay::{ReplayError, ReplayFrame, ReplayLog};

#[test]
fn replay_log_records_initial_state_inputs_and_checksums() {
    let initial = World::for_two_players();
    let inputs = [
        PlayerInput::neutral().with_left_stick(64, 0),
        PlayerInput::neutral(),
    ];
    let mut world = initial.clone();
    step_world(&mut world, Frame(0), &inputs);
    let mut log = ReplayLog::new(initial.clone());

    log.push(ReplayFrame {
        frame: Frame(0),
        inputs,
        checksum: world.checksum(),
    });

    assert_eq!(log.initial().checksum(), initial.checksum());
    assert_eq!(log.frames().len(), 1);
    assert_eq!(log.frames()[0].frame, Frame(0));
    assert_eq!(log.frames()[0].inputs, inputs);
    assert_eq!(log.frames()[0].checksum, world.checksum());
}

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
fn replay_log_replays_recorded_inputs_to_same_final_checksum() {
    let initial = World::for_two_players();
    let mut world = initial.clone();
    let inputs_by_frame = [
        [
            PlayerInput::neutral().with_left_stick(64, 0),
            PlayerInput::neutral(),
        ],
        [
            PlayerInput::neutral().with_left_stick(90, 0),
            PlayerInput::neutral().with_left_stick(-64, 0),
        ],
        [
            PlayerInput::neutral().with_attack(true),
            PlayerInput::neutral(),
        ],
    ];
    let mut log = ReplayLog::new(initial);

    for (frame_index, inputs) in inputs_by_frame.into_iter().enumerate() {
        let frame = Frame(frame_index as u32);
        step_world(&mut world, frame, &inputs);
        log.push(ReplayFrame {
            frame,
            inputs,
            checksum: world.checksum(),
        });
    }

    let replayed = log.replay().expect("recorded replay should validate");

    assert_eq!(replayed.checksum(), world.checksum());
    assert_eq!(replayed.frame(), world.frame());
}

#[test]
fn replay_log_detects_changed_input_checksum_mismatch() {
    let initial = World::for_two_players();
    let mut expected_world = initial.clone();
    let expected_inputs = [
        PlayerInput::neutral().with_left_stick(127, 0),
        PlayerInput::neutral(),
    ];
    step_world(&mut expected_world, Frame(0), &expected_inputs);
    let mut log = ReplayLog::new(World::for_two_players());

    log.push(ReplayFrame {
        frame: Frame(0),
        inputs: [PlayerInput::neutral(), PlayerInput::neutral()],
        checksum: expected_world.checksum(),
    });

    match log.validate() {
        Err(ReplayError::ChecksumMismatch {
            frame,
            expected,
            actual,
        }) => {
            assert_eq!(frame, Frame(0));
            assert_eq!(expected, expected_world.checksum());
            assert_ne!(actual, expected_world.checksum());
        }
        Ok(()) => panic!("changed input should fail replay validation"),
    }
}
