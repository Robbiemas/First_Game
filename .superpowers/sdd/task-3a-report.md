# Task 3A Report: Rollback-Owned Source AObj Playback

## Status

Complete.

Feature implementation commit: `05329202742c7d4ab699b99c99ef640b4741f09c` (`Persist source AObj playback state`)

## TDD Evidence

The inherited `state.rs` draft already contained the persistent playback implementation and its core focused tests. A targeted compatibility regression test was added before correcting the inherited draft.

1. RED: added `legacy_animation_frame_setters_keep_primary_playback_synchronized`.
   - Command: `cargo test -p mole_core legacy_animation_frame_setters_keep_primary_playback_synchronized --lib`
   - Result: failed as expected because `set_source_motion_anim_frame_milli(3_250)` left `source_playback.primary.curr_frame` at `0.0` rather than `3.25`.
   - Counts: 0 passed, 1 failed, 75 filtered out.

2. GREEN: synchronized the millisecond legacy frame setter with the persistent primary AObj.
   - Command: `cargo test -p mole_core legacy_animation_frame_setters_keep_primary_playback_synchronized --lib`
   - Result: passed.
   - Counts: 1 passed, 0 failed, 75 filtered out.

## Implementation

- Added rollback-owned `SourceFighterPlayback` with primary and optional secondary `SourceAObjState` values.
- Stored playback on `PlayerState`, included it in `PlayerRollbackSnapshot`, and mixed every primary and secondary playback field into `World::checksum`.
- Added narrow primary request/interpret and secondary access methods for the Task 3B call-site migration.
- Kept the legacy frame/rate fields synchronized when playback advances and when the existing frame/rate setters are used.
- Added focused coverage for persistent request/first-play/loop/stop behavior, legacy synchronization, rollback round-trip, and checksum sensitivity for every stored playback field.

## Files Changed

- `crates/mole_core/src/state.rs`

## Verification

- `cargo run -p mole_cli -- tests run -p mole_core persistent_fighter_playback_requests_interprets_loops_and_stops legacy_animation_frame_setters_keep_primary_playback_synchronized persistent_fighter_playback_round_trips_through_rollback checksum_is_sensitive_to_every_persistent_playback_field`: passed; all four exact filters passed.
- `cargo fmt -p mole_core -- --check`: passed.
- `cargo test -p mole_core --lib`: passed; 76 passed, 0 failed.
- `git diff --check`: passed.

## Migration Caveat

Task 3A deliberately does not rewrite `sim.rs` callback dispatch or its remaining direct legacy clock/rate writes. Task 3B must route those call sites through the new playback request/interpret API so the persistent AObj becomes the sole advancing authority. No runtime, replay, simulation, or generated files were changed here.
