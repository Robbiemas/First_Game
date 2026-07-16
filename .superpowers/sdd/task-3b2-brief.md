# Task 3B2: Global priority-3 input family

## Goal

Publish the decomp-shaped global priority-3 fighter input phase and migrate the
coherent AttackAir-entry and Turn redispatch family. Reuse the priority-1 AObj
evaluator rather than adding action-local frame sampling or replay behavior.

## Source contract

- `fighter.c:906` and `fighter.c:1696-1706`: fighter GObj callbacks execute in
  global priority order; later phases reread the live callback after transitions.
- `ftCo_AttackAir.c:127-151`: aerial attack selection changes motion state,
  explicitly evaluates the new animation once, and does not recursively call the
  new animation callback; the ordinary callback later handles facing flags and
  AObj completion.
- `ftCo_Turn.c:51-96`: Turn entry explicitly evaluates animation; priority 1
  decrements `frames_to_turn`, flips once, and publishes `just_turned`.
- `ftCo_Turn.c:99-150`: priority 3 ORs latched A/B presses into current input on
  the turn frame, evaluates current-stick actions in source order, latches current
  A/B only after higher-priority checks fail, then clears `just_turned`.

## Required architecture

- Add a stable-player-order global p3 loop after the complete p1 phase and before
  p4/p6 work. Dispatch from the live action/input callback for each player.
- Refactor one callable active-animation evaluator shared by ordinary p1 and
  explicit entry helpers. It advances the installed AObj and command stream once;
  callback dispatch remains a separate operation.
- AttackAir entry from p3 must install the exact source action descriptor and run
  the same explicit evaluation sequence as `Fighter_ChangeMotionState` followed by
  `ftAnim_8006EBA4`. It must not invoke the new AttackAir anim callback recursively.
- Remove `sample_source_motion_frame_for_action_entry` where the shared evaluator
  now owns the source behavior. Do not preserve a second advancement path.
- Turn p1 owns decrement/flip/`just_turned`. Turn p3 combines latched A/B with the
  current controller row while preserving the current stick, follows source input
  priority, and latches current A/B only after those checks fail.
- Keep phase scratch fixed-size and ephemeral. Persistent source fields remain
  rollback-owned and checksummed; no test-name, replay-frame, or tolerance logic.

## Acceptance tests

- `aerial_attack_iasa_advances_new_action_anim_on_entry_tick`
- `neutral_special_latched_during_turn_replays_with_current_stick_on_turn_frame`

Add focused contracts for stable p3 player order, explicit AttackAir entry
evaluation without recursive anim callback, and Turn latch timing if existing
coverage is insufficient. Keep the RebirthWait and DamageFly scheduler contracts
RED only for their known p4/p6 gaps.

Run focused contracts, `cargo test -p mole_core --lib`, formatting and diff checks,
then the canonical strict replay gate for all 5,313 comparisons. Any divergence is
a blocker.

## Ownership and constraints

- Own `crates/mole_core/src/sim.rs`, `crates/mole_core/src/state.rs` only if a
  source-owned persistent Turn field requires it, focused tests, and a Task 3B2
  report.
- Inspect source through Mole CLI before editing and use TDD.
- No runtime/replay special cases, resync, tolerances, generated-data edits,
  profile timing substitutes, or action-local animation clocks.
- Commit code/tests and report separately. You are not alone in the codebase; do
  not revert unrelated changes.
