# Pygame Movement Logic Pass

This note tracks the bottom-up Pygame movement audit. The long-term direction is
still to make the Rust core authoritative, but the Pygame layer is the current
playable test surface.

## Reference Used

- Local Rust core state interpreter: `crates/mole_core/src/sim.rs`
- Local Rust state tests: `crates/mole_core/tests/core_contract.rs`
- State inventory comparison: `docs/research/mole-state-coverage-comparison.md`

## Fixed In This Pass

### Walk opposite soft stick

Reference behavior:

- `Walk` plus a soft opposite stick exits to `Wait`.
- Facing does not flip.
- Velocity is cleared by the state transition.

Pygame behavior before this pass:

- `walking` plus any opposite stick entered `turning`.
- The same frame could continue through later walking logic.

Current behavior:

- `walking` plus opposite stick exits to `standing`.
- Facing is preserved.
- Horizontal velocity is cleared.
- A local resolver guard prevents the same frame from re-entering `walking`.

### TurnRun facing and jump cancel

Reference behavior:

- `Run` plus hard opposite stick enters `TurnRun`.
- Facing stays old-facing until horizontal velocity crosses zero.
- A normal jump can interrupt `TurnRun`.

Pygame behavior before this pass:

- `runTurn` flipped facing immediately on entry.
- `runTurn` blocked jump by forcing `canJump = False`.
- `runTurn` waited on a fixed long counter before allowing run/walk exit.

Current behavior:

- `runTurn` keeps old-facing on entry.
- Opposite stick applies acceleration toward the new direction.
- Facing flips when velocity crosses zero.
- Jump can enter `jumpSquat` from `runTurn`.
- Entry-frame traction is preserved without applying duplicate run-turn
  traction on later frames.

### UCF dashback boundary

Reference behavior:

- UCF acts as an input wrapper before the engine consumes the frame.
- Gameplay movement should consume canonical Melee facts, not UCF-specific
  action flags.

Pygame behavior before this pass:

- The Rust stream exposed `ucf_dashback_direction`.
- Pygame dash checks only consumed vanilla `dash_direction`, so UCF-corrected
  dashback could be visible in input data but ignored by movement.

Current behavior:

- Rust folds UCF dashback into canonical `dash_direction` in
  `MeleeInputSnapshot::facts()`.
- The Python input bridge folds the same fact for compatibility with older
  runtime output.
- `Character.has_fresh_x_tap()` only checks canonical `melee_dash_direction`.

## Tests Added

- `test_walk_soft_opposite_stick_exits_to_wait_without_flipping_facing`
- `test_run_turn_keeps_old_facing_until_velocity_crosses_zero`
- `test_run_turn_allows_jump_cancel_before_long_turn_window`
- `test_native_ucf_dashback_fact_allows_dash_dance_when_vanilla_timer_misses`

## Remaining Pygame Layer Suspects

These are not fixed in this pass, but they are likely sources of future
movement-feel mismatch:

- `running` still uses `endLag` as a `RunBrake` substitute instead of an
  explicit state.
- `blocking` still collapses `GuardOn` and `Guard`; Rust separates these.
- `air` still collapses jump, fall, and aerial drift variants.
- `walking` still has slow/middle/fast as flags rather than explicit state
  buckets or state metadata.
- Pygame still has several same-frame overwrite paths because
  `resolve_action_state()` is a single large resolver rather than per-state
  transition handlers.
