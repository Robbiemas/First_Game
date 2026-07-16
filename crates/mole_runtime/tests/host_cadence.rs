use mole_core::{Frame, PlayerInput, World};
use mole_runtime::{step_world_with_source_collisions, HostCadence, HostCadenceClock, HostPassId};

const GAME_FRAMES: u32 = 180;

fn inputs_for(frame: Frame) -> [PlayerInput; 2] {
    let phase = (frame.0 % 48) as i8;
    [
        PlayerInput::neutral().with_left_stick(phase.saturating_mul(2), 0),
        PlayerInput::neutral().with_left_stick(-phase.saturating_mul(2), 0),
    ]
}

fn run(cadence: HostCadence) -> (u32, u64) {
    let mut clock = HostCadenceClock::new(cadence);
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let mut game_frames = 0;

    while game_frames < GAME_FRAMES {
        let pass = clock.advance();
        if let Some(frame) = pass.simulation_frame() {
            step_world_with_source_collisions(&mut world, frame, &inputs_for(frame));
            game_frames += 1;
        }
    }

    (game_frames, world.checksum())
}

#[test]
fn cadence_multipliers_and_host_pass_ids_are_explicit() {
    assert_eq!(HostCadence::Hz60.multiplier(), 1);
    assert_eq!(HostCadence::Hz120.multiplier(), 2);
    assert_eq!(HostCadence::Hz180.multiplier(), 3);
    assert_eq!(HostCadence::Hz240.multiplier(), 4);

    let mut clock = HostCadenceClock::new(HostCadence::Hz240);
    assert_eq!(clock.advance().id(), HostPassId(0));
    assert_eq!(clock.advance().id(), HostPassId(1));
}

#[test]
fn all_host_cadences_produce_identical_game_frames_and_checksum() {
    let expected = run(HostCadence::Hz60);

    for cadence in [HostCadence::Hz120, HostCadence::Hz180, HostCadence::Hz240] {
        assert_eq!(run(cadence), expected, "cadence {cadence:?}");
    }
}

#[test]
fn peers_with_mixed_host_cadences_reach_the_same_authoritative_state() {
    for (left, right) in [
        (HostCadence::Hz60, HostCadence::Hz120),
        (HostCadence::Hz120, HostCadence::Hz180),
        (HostCadence::Hz180, HostCadence::Hz240),
        (HostCadence::Hz60, HostCadence::Hz240),
    ] {
        assert_eq!(run(left), run(right), "mixed peers {left:?}/{right:?}");
    }
}

#[test]
fn cadence_changes_preserve_monotonic_host_and_simulation_clocks() {
    let mut clock = HostCadenceClock::new(HostCadence::Hz240);
    let passes = (0..4).map(|_| clock.advance()).collect::<Vec<_>>();
    assert_eq!(passes.last().unwrap().simulation_frame(), Some(Frame(0)));

    clock.set_cadence_at_simulation_boundary(HostCadence::Hz120);
    let first = clock.advance();
    let second = clock.advance();

    assert_eq!(first.id().0, 4);
    assert_eq!(second.id().0, 5);
    assert_eq!(first.simulation_frame(), None);
    assert_eq!(second.simulation_frame(), Some(Frame(1)));
}
