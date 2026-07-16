# Task 3B1 Report

## Status

Code commit: `48edae3` (`Add global source animation phase`)

Task 3B1 is complete. The strict sequential match-start replay gate is exact:
6,000 requested frames, 5,313 compared frames, zero state mismatches, and no
classified divergence.

## Implementation

- Added a stable global priority-1 animation phase before the existing
  per-player work. It evaluates player 0 before player 1 and dispatches the
  live Dash and Attack100 callbacks once per phase.
- Migrated Dash and Attack100 Start/Loop/End to rollback-owned primary AObj
  playback. The old per-state animation advancement is skipped for those
  states; `motion_frame` remains an action-command timeline.
- Bound migrated AObjs through action ID plus source action key. This keeps
  common `Attack1` runtime aliases distinct and leaves the legacy action-table
  ECB pose path unchanged.
- Modeled Dash entry as the source `Fighter_ChangeMotionState` first-play
  evaluation followed by Dash_Enter's explicit `ftAnim_8006EBA4` evaluation.
  The explicit entry advance consumes the first of Dash's 29 AObj frames, so
  the two Dash contracts now assert the source-accurate completion row.
- Used AObj `NO_ANIM` state for Dash and Attack100 completion. Start-to-Loop,
  Loop B3 dispatch, and End completion do not use profile or replay frame
  counts.

## Divergence Investigation

The inherited draft first diverged at source frame 397: core still reported
Dash while source had reached Wait. Trace evidence showed that entry playback
was one AObj evaluation behind. Adding the source ChangeMotionState first-play
evaluation plus Dash_Enter's explicit evaluation moved the first divergence to
source frame 2278.

That later mismatch was a separate regression from applying the new
identity-aware descriptor lookup to legacy ECB pose selection. DamageN2 then
sampled a Dash pose and landed early. The final implementation isolates the
migrated playback binding from the legacy pose resolver; the strict gate is
exact without resync, tolerance, replay-only behavior, or generated-data
changes.

## Verification

- Mole CLI focused target contracts: all passed.
  - `dash_neutral_falls_back_on_animation_completion_not_profile_dash_frames`
  - `ground_jump_takeoff_carries_moonwalk_followthrough_slide_without_custom_state`
  - `attack100_start_loop_and_end_follow_decomp_action_chain`
- Focused p1/identity tests: passed, including entry first-play/explicit
  evaluation and non-recursive transition dispatch.
- `cargo test -p mole_core --lib`: passed, 81 passed / 0 failed.
- The four remaining scheduler contracts are still red for their existing
  unmigrated phase gaps: AttackAir entry, Turn input redispatch, RebirthWait
  p4 handoff, and DamageFly exit.
- `cargo fmt --all -- --check`: passed.
- `git diff --check`: passed before the code commit.
- Strict replay check: passed, 5,313 comparisons / 0 state mismatches.

## Scope

- `crates/mole_core/src/state.rs`
- `crates/mole_core/src/sim.rs`
- `crates/mole_core/tests/core_contract.rs`
