use mole_core::step_world;
use mole_core::{Frame, PlayerInput, World};
use mole_rollback::{RollbackSession, SnapshotBuffer};

#[test]
fn snapshot_buffer_restores_by_frame() {
    let mut buffer = SnapshotBuffer::new(8);
    let world = World::for_two_players();

    buffer.save(Frame(7), &world);

    assert_eq!(buffer.load(Frame(7)).unwrap().checksum(), world.checksum());
}

#[test]
fn snapshot_buffer_wraps_without_returning_stale_frames() {
    let mut buffer = SnapshotBuffer::new(2);
    let world = World::for_two_players();

    buffer.save(Frame(0), &world);
    buffer.save(Frame(1), &world);
    buffer.save(Frame(2), &world);

    assert!(buffer.load(Frame(0)).is_none());
    assert!(buffer.load(Frame(2)).is_some());
}

#[test]
fn corrected_input_resimulates_to_different_checksum() {
    let initial = World::for_two_players();
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let corrected = [
        PlayerInput::neutral().with_left_stick(127, 0),
        PlayerInput::neutral(),
    ];
    let mut session = RollbackSession::new(initial, 32);

    session.advance(Frame(0), neutral);
    let before = session.world().checksum();
    session.correct_and_resimulate(Frame(0), corrected, Frame(1));

    assert_ne!(session.world().checksum(), before);
}

#[test]
fn missing_remote_input_is_predicted_from_previous_frame() {
    let local = PlayerInput::neutral().with_left_stick(64, 0);
    let remote = PlayerInput::neutral().with_left_stick(-64, 0);
    let mut session = RollbackSession::new(World::for_two_players(), 32);

    let frame_zero = session.advance_with_prediction(Frame(0), [Some(local), Some(remote)]);
    let frame_one = session.advance_with_prediction(Frame(1), [Some(local), None]);

    assert_eq!(frame_zero, [local, remote]);
    assert_eq!(frame_one, [local, remote]);
}

#[test]
fn confirmed_matching_prediction_does_not_resimulate() {
    let remote = PlayerInput::neutral().with_left_stick(-64, 0);
    let mut session = RollbackSession::new(World::for_two_players(), 32);

    session.advance_with_prediction(Frame(0), [Some(PlayerInput::neutral()), Some(remote)]);
    session.advance_with_prediction(Frame(1), [Some(PlayerInput::neutral()), None]);
    let before = session.world().checksum();

    let resimulated = session.confirm_input(Frame(1), 1, remote, Frame(2));

    assert!(!resimulated);
    assert_eq!(session.world().checksum(), before);
}

#[test]
fn corrected_remote_input_resimulates_to_no_delay_authoritative_checksum() {
    let local = PlayerInput::neutral().with_left_stick(64, 0);
    let predicted_remote = PlayerInput::neutral();
    let actual_remote = PlayerInput::neutral().with_left_stick(-127, 0);
    let frame_two_remote = PlayerInput::neutral().with_attack(true);
    let frame_zero_inputs = [local, predicted_remote];
    let frame_one_predicted = [local, predicted_remote];
    let frame_one_actual = [local, actual_remote];
    let frame_two_inputs = [PlayerInput::neutral(), frame_two_remote];
    let initial = World::for_two_players();
    let mut authoritative = initial.clone();
    let mut session = RollbackSession::new(initial, 32);

    step_world(&mut authoritative, Frame(0), &frame_zero_inputs);
    step_world(&mut authoritative, Frame(1), &frame_one_actual);
    step_world(&mut authoritative, Frame(2), &frame_two_inputs);

    session.advance_with_prediction(Frame(0), [Some(local), Some(predicted_remote)]);
    let predicted = session.advance_with_prediction(Frame(1), [Some(local), None]);
    assert_eq!(predicted, frame_one_predicted);
    session.advance_with_prediction(
        Frame(2),
        [Some(PlayerInput::neutral()), Some(frame_two_remote)],
    );

    let resimulated = session.confirm_input(Frame(1), 1, actual_remote, Frame(3));

    assert!(resimulated);
    assert_eq!(session.world().checksum(), authoritative.checksum());
}
