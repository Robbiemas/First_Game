# Task 3B1 Report

## Status

Task 3B1 review remediation is complete. The canonical strict replay remains
exact: 6,000 requested frames, 5,313 compared frames, zero state mismatches,
and no classified divergence.

## Remediation

- Attack100Start now mirrors `ftCo_800D6B00`: motion-state installation runs
  the new AObj/script once for `Fighter_ChangeMotionState`, then the entry helper
  runs the explicit `ftAnim_8006EBA4` evaluation. Neither entry evaluation calls
  the newly installed animation callback.
- Start-to-Loop and Loop-to-End now install and immediately evaluate the new
  AObj/script once, matching their direct `Fighter_ChangeMotionState` calls.
  The captured old callback returns without recursively dispatching the new one.
- Loop-start state is published by the first ordinary Loop p1 callback, not by
  transition-time evaluation. `motion_frame` remains the integer command
  timeline and does not advance during entry evaluation.
- The shared evaluation primitive owns migrated AObj evaluation plus script
  application for ordinary p1 work and explicit entry calls.

## Source Evidence

- `ftCo_Attack100.c:247-256`: Start entry clears throw flags, calls
  `Fighter_ChangeMotionState`, explicitly calls `ftAnim_8006EBA4`, then clears
  the two Attack100 move-state booleans.
- `ftCo_Attack100.c:265-271`: Start completion changes directly to Loop without
  an explicit `ftAnim` call or recursive Loop callback.
- `ftCo_Attack100.c:299-323`: Loop callback consumes B3 and changes directly to
  End when the loop has started and continuation input is absent.
- `ftanim.c:381-387`: `ftAnim_8006EBA4` evaluates animation before processing
  the fighter command script.
- Checked-in Falcon extraction binds `Attack100Loop` to action-table index 50,
  distinct from common state ID 48.
- Checked-in Falcon extraction binds `Attack100Loop` to subaction offset
  `18192` (`0x4710`). Inspection of that checked-in `PlCa.dat` command stream
  and its `0x46BC` subroutine establishes `SetThrowFlag(hit_idx=0)` at frames
  6, 14, 22, 30, and 37. A Mole CLI decoder contract reproduces those exact
  wait/call/subroutine commands, while the compact core fixture uses the same
  source-keyed rows. The old common-ID-48/frame-40 fixture was removed.

Source artifact hashes:

- `resources/melee/raw/PlCa.dat`: SHA-256
  `4CF61A52737D464DF9298FD15573345FB3B9A15C79AB47DCE4FD2E3E707917AF`,
  Git blob `01f46248a0bd9033f880274c46b995d15aec29d1`.
- `resources/melee/extracted/captain_falcon_action_animation_table.json`:
  SHA-256
  `F4CEECB4426C0D8AA149878879058C4C8353359CAB8E7130B2A73F963EB97C92`,
  Git blob `307417a179aa29598985a74a38e88eaf13f0e8b9`.

## TDD Evidence

- RED: exact Start entry expected AObj frame 1 but observed frame 0.
- RED: exact Loop transition expected immediate evaluation with first-play
  cleared, and the chain remained in Loop under the old frame-40 fixture.
- RED: transition-time Loop callback state was incorrectly published before the
  first ordinary Loop p1 callback.
- GREEN: focused Start entry, Start-to-Loop, Loop-to-End, no-recursion, Falcon
  source-proof fixture, Dash completion, and moonwalk carry contracts all pass.

## Verification

- Focused Mole CLI contracts: 6 passed / 0 failed, covering exact Start entry,
  the full Attack100 chain, non-recursive transitions, checked-in source proof,
  Dash completion, and moonwalk carry.
- `cargo test -p mole_core --lib`: 81 passed / 0 failed.
- `cargo build -p mole_cli`: passed.
- `cargo fmt --all -- --check`: passed.
- `git diff --check`: passed.
- Strict replay check: `ok: true`, 5,313 comparisons, 0 state mismatches,
  no position drift, no classified divergence.

## Scope

- `crates/mole_core/src/sim.rs`
- `crates/mole_core/tests/core_contract.rs`
- `.superpowers/sdd/task-3b1-report.md`
- `.superpowers/sdd/progress.md`
