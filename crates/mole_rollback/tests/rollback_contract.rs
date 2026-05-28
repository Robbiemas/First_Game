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
