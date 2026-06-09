use mole_core::step_world;
use mole_core::{
    FighterProfile, Frame, MeleeCommonData, PlayerInput, StageProfile, World, PLAYER_COUNT,
};
use mole_rollback::{InputDelayBuffer, RollbackSession, SlippiInputDelayBuffer, SnapshotBuffer};

#[test]
fn snapshot_buffer_restores_by_frame() {
    let mut buffer = SnapshotBuffer::new(8);
    let world = World::for_two_players();
    let mut restored = World::for_two_players();

    buffer.save(Frame(7), &world);

    assert!(buffer.restore(Frame(7), &mut restored));
    assert_eq!(restored.checksum(), world.checksum());
}

#[test]
fn snapshot_buffer_wraps_without_returning_stale_frames() {
    let mut buffer = SnapshotBuffer::new(2);
    let world = World::for_two_players();

    buffer.save(Frame(0), &world);
    buffer.save(Frame(1), &world);
    buffer.save(Frame(2), &world);

    let mut restored = World::for_two_players();
    assert!(!buffer.restore(Frame(0), &mut restored));
    assert!(buffer.restore(Frame(2), &mut restored));
}

#[test]
fn snapshot_buffer_restores_authoritative_state_without_overwriting_static_world_config() {
    let mut source_stage = StageProfile::battlefield_test();
    source_stage.name = "source_stage";
    source_stage.main_floor.y = 123;
    let mut target_stage = StageProfile::battlefield_test();
    target_stage.name = "target_stage";
    target_stage.main_floor.y = -321;

    let mut source_common = MeleeCommonData::provisional_mole();
    source_common.dash_x = 101;
    let mut target_common = MeleeCommonData::provisional_mole();
    target_common.dash_x = 64;

    let mut source_profile = FighterProfile::FALCON_LIKE;
    source_profile.weight = 104.0;
    let mut target_profile = FighterProfile::FALCON_LIKE;
    target_profile.weight = 88.0;

    let mut source = World::for_two_players_on_stage_with_profiles_and_common_data(
        source_stage,
        [source_profile; PLAYER_COUNT],
        source_common,
    );
    step_world(
        &mut source,
        Frame(0),
        &[
            PlayerInput::neutral().with_left_stick(127, 0),
            PlayerInput::neutral(),
        ],
    );

    let mut target = World::for_two_players_on_stage_with_profiles_and_common_data(
        target_stage,
        [target_profile; PLAYER_COUNT],
        target_common,
    );
    let target_stage_before = target.stage();
    let target_common_before = target.common_data();
    let target_profiles_before = [target.players()[0].profile, target.players()[1].profile];

    let mut divergent_player = target.players()[0];
    divergent_player.position.x += 10_000;
    assert!(target.set_player_state_for_diagnostic(0, divergent_player));

    let mut buffer = SnapshotBuffer::new(8);
    buffer.save(Frame(4), &source);

    assert!(buffer.restore(Frame(4), &mut target));
    assert_eq!(target.stage(), target_stage_before);
    assert_eq!(target.common_data(), target_common_before);
    assert_eq!(target.players()[0].profile, target_profiles_before[0]);
    assert_eq!(target.players()[1].profile, target_profiles_before[1]);
    assert_eq!(target.frame(), source.frame());
    assert_eq!(target.players()[0].position, source.players()[0].position);
    assert_eq!(
        target.players()[0].motion_state,
        source.players()[0].motion_state
    );
    assert_eq!(
        target.players()[0].melee_action_state_id,
        source.players()[0].melee_action_state_id
    );
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

#[test]
fn input_delay_buffer_outputs_inputs_after_configured_frame_delay() {
    let mut delay = InputDelayBuffer::new(2);
    let first = PlayerInput::neutral().with_attack(true);
    let second = PlayerInput::neutral().with_special(true);
    let third = PlayerInput::neutral().with_left_stick(127, 0);

    assert_eq!(
        delay.push_and_get_committed(Frame(0), first),
        PlayerInput::neutral()
    );
    assert_eq!(
        delay.push_and_get_committed(Frame(1), second),
        PlayerInput::neutral()
    );
    assert_eq!(delay.push_and_get_committed(Frame(2), third), first);
    assert_eq!(
        delay.push_and_get_committed(Frame(3), PlayerInput::neutral()),
        second
    );
}

#[test]
fn slippi_input_delay_schedules_physical_input_on_future_game_frame() {
    let mut delay = SlippiInputDelayBuffer::new(2);
    let first = PlayerInput::neutral().with_attack(true);
    let second = PlayerInput::neutral().with_special(true);
    let third = PlayerInput::neutral().with_left_stick(127, 0);

    let frame_zero = delay.push_physical_input(Frame(0), first);
    assert_eq!(frame_zero.scheduled_frame, Frame(2));
    assert_eq!(frame_zero.scheduled_input, first);
    assert_eq!(frame_zero.current_frame_input, PlayerInput::neutral());

    let frame_one = delay.push_physical_input(Frame(1), second);
    assert_eq!(frame_one.scheduled_frame, Frame(3));
    assert_eq!(frame_one.scheduled_input, second);
    assert_eq!(frame_one.current_frame_input, PlayerInput::neutral());

    let frame_two = delay.push_physical_input(Frame(2), third);
    assert_eq!(frame_two.scheduled_frame, Frame(4));
    assert_eq!(frame_two.scheduled_input, third);
    assert_eq!(frame_two.current_frame_input, first);
}

#[test]
fn stale_confirmed_input_outside_snapshot_window_does_not_panic_or_resimulate() {
    let mut session = RollbackSession::new(World::for_two_players(), 2);
    session.advance_with_prediction(
        Frame(0),
        [
            Some(PlayerInput::neutral()),
            Some(PlayerInput::neutral().with_attack(true)),
        ],
    );
    session.advance_with_prediction(Frame(1), [Some(PlayerInput::neutral()), None]);
    session.advance_with_prediction(Frame(2), [Some(PlayerInput::neutral()), None]);
    let before = session.world().checksum();

    let resimulated = session.confirm_input(
        Frame(0),
        1,
        PlayerInput::neutral().with_special(true),
        Frame(3),
    );

    assert!(!resimulated);
    assert_eq!(session.world().checksum(), before);
}
