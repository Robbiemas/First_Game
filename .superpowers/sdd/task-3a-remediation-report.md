# Task 3A Remediation Report

## Status

Code commit: `03dfb92` (`Correct persistent AObj playback semantics`)

## TDD Evidence

1. RED: added the distinct install/request API and milli-rate setter coverage. The Mole CLI focused run failed to compile because `install_primary_descriptor`, `request_primary_frame`, and `set_source_motion_anim_rate_milli` did not exist.
2. GREEN: the same focused checks passed after the narrow APIs and synchronized production write migration.
3. RED: removed diagnostic playback reconciliation and added `diagnostic_state_normalization_reconciles_primary_playback`; it failed with primary frame `9.0` instead of legacy frame `3.25`.
4. GREEN: restoring explicit reconciliation made that diagnostic test pass.

## Changes

- Separated `install_primary_descriptor` from `request_primary_frame`. The request operation now matches `HSD_AObjReqAnim`: set `curr_frame`, clear only `NO_ANIM`, add `FIRST_PLAY`, and preserve every other flag and descriptor field.
- Added synchronized milli-rate mutation and routed all production frame/rate mutations in `state.rs` and `sim.rs` through the existing synchronized frame setters or the new rate setter.
- Explicitly reconciled primary playback during diagnostic normalization.
- Extended checksum coverage to independently vary every secondary AObj field: flags, current frame, rewind frame, end frame, and frame rate.
- No scheduler branches, replay behavior, tolerances, or generated data changed.

## Verification

- Mole CLI focused playback/remediation filters: passed.
- `cargo fmt --all -- --check`: passed.
- `cargo test -p mole_core --lib`: passed, 78 passed / 0 failed.
- `git diff --check`: passed before the code commit.
- Scheduler baseline filters were run via Mole CLI. Their RED checks remain non-blocking for this mechanical migration; no scheduler behavior was modified. The completed candidate run also included two green filters, so those names were not treated as evidence that the scheduler baseline had been fixed.

## Files Changed

- `crates/mole_core/src/state.rs`
- `crates/mole_core/src/sim.rs`
