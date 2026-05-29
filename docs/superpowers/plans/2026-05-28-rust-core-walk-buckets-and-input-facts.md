# Rust Core Walk Buckets And Input Facts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the Rust core own explicit `Wait -> WalkSlow/WalkMiddle/WalkFast` transitions from rollback-owned input facts.

**Architecture:** `mole_core` remains deterministic and authoritative. The input snapshot derives a walk bucket fact from current stick magnitude and `MeleeCommonData` thresholds, and the state-local grounded transition code chooses explicit walk motion states while preserving existing walk physics and priority. Pygame remains untouched.

**Tech Stack:** Rust 2021, Cargo workspace, `mole_core` contract tests, Markdown docs.

---

## Scope Guard

Work from `D:\Mole Game\First_Game` on branch `handoff/rust-rollback-architecture`.
In Codex desktop, `apply_patch` is rooted one level up at `D:\Mole Game`, so patch paths must start with `First_Game/`.

Baseline at plan creation:

- `git -C "D:\Mole Game\First_Game" status -sb`: clean on `handoff/rust-rollback-architecture`
- `cargo test --workspace`: passing before this slice
- `python -m pytest`: blocked by missing `pygame` in the active Python environment; do not repair Pygame for this slice

## Files

- Modify: `crates/mole_core/src/input.rs`
- Modify: `crates/mole_core/src/common_data.rs`
- Modify: `crates/mole_core/src/lib.rs`
- Modify: `crates/mole_core/src/state.rs`
- Modify: `crates/mole_core/src/sim.rs`
- Test: `crates/mole_core/tests/core_contract.rs`
- Update: `docs/research/melee-input-state-reference.md`
- Update: `docs/research/mole-state-coverage-comparison.md`

## Task 1: Add Walk Bucket Input Fact

- [x] Add `WalkSpeedBucket::{None, Slow, Middle, Fast}` to `crates/mole_core/src/input.rs`.
- [x] Add `walk_speed_bucket: WalkSpeedBucket` to `MeleeInputFacts`.
- [x] Derive the bucket from current `lstick.0` inside `MeleeInputSnapshot::facts`.
- [x] Move bucket cutoffs into `MeleeCommonData` and `MeleeInputThresholds` instead of hardcoding them in the helper.
- [x] Initially source-mapped `walk_slow_x`, `walk_middle_x`, and `walk_fast_x` to `x28`, `x2C`, and `x30`.
  Corrected on 2026-05-29: decomp shows `x28`/`x2C` are walk velocity ratios and `x30` is walk acceleration taper, so Rust walk input buckets are now marked provisional instead of source-mapped to those fields.
- [x] Export `WalkSpeedBucket` from `crates/mole_core/src/lib.rs`.
- [x] Add a failing contract test named `melee_input_facts_classify_walk_speed_bucket_from_current_stick`.
- [x] Add a failing contract test named `melee_input_facts_classify_walk_speed_bucket_from_common_data_thresholds`.
- [x] Verify RED with `cargo test -p mole_core melee_input_facts_classify_walk_speed_bucket_from_current_stick`.
- [x] Implement the data-driven helper:

```rust
fn walk_speed_bucket(stick_x: i8, thresholds: MeleeInputThresholds) -> WalkSpeedBucket {
    let magnitude = stick_x.saturating_abs() as i16;
    let slow_threshold =
        threshold_abs(thresholds.walk_slow_x).max(threshold_abs(thresholds.walk_x));
    let middle_threshold = threshold_abs(thresholds.walk_middle_x).max(slow_threshold);
    let fast_threshold = threshold_abs(thresholds.walk_fast_x).max(middle_threshold);

    if magnitude < slow_threshold {
        WalkSpeedBucket::None
    } else if magnitude < middle_threshold {
        WalkSpeedBucket::Slow
    } else if magnitude < fast_threshold {
        WalkSpeedBucket::Middle
    } else {
        WalkSpeedBucket::Fast
    }
}
```

- [x] Verify GREEN with `cargo test -p mole_core melee_input_facts_classify_walk_speed_bucket_from_current_stick`.

## Task 2: Split Walk Motion State Identity

- [x] Add failing tests for:
  - `wait_enters_walk_slow_from_soft_forward_stick`
  - `wait_enters_walk_middle_from_mid_forward_stick`
  - `wait_enters_walk_fast_after_dash_tap_window_expires`
- [x] Verify RED with `cargo test -p mole_core wait_enters_walk_`.
- [x] Replace `MotionState::Walk` with `WalkSlow`, `WalkMiddle`, and `WalkFast`.
- [x] Update `motion_state_id` so every state has a unique checksum id.
- [x] Change walk entry to call `enter_walk(player, input_facts.walk_speed_bucket, stick_x)`.
- [x] Change the walk match arm to cover all three walk states.
- [x] Refresh the current walk state bucket while continuing walk.
- [x] Update dash exit and grounded IASA walk entry to use the current walk bucket fact.
- [x] Update existing tests that expected `MotionState::Walk`.
- [x] Verify GREEN with:

```powershell
cargo test -p mole_core wait_enters_walk_
cargo test -p mole_core walk_state_
```

## Task 2.5: Route Walk Physics Through Fighter Profile Data

- [x] Add a failing contract test named `walk_velocity_uses_player_profile_attributes`.
- [x] Add deterministic `FighterProfile` walk fields for target speed, initial acceleration, walk acceleration, and walk friction.
- [x] Seed `World::for_two_players` with the default Falcon-like profile.
- [x] Add `World::for_two_players_with_profiles` so tests and future replay/session setup can choose deterministic profiles explicitly.
- [x] Include profile walk fields in the world checksum.
- [x] Route `apply_walk_velocity` through the player's profile instead of file-local walk constants.
- [x] Verify GREEN with `cargo test -p mole_core walk_velocity_uses_player_profile_attributes`.

## Task 3: Update Coverage Docs

- [x] Update the Rust state list in `docs/research/mole-state-coverage-comparison.md` to include `WalkSlow`, `WalkMiddle`, and `WalkFast`.
- [x] Update the `WalkSlow/Middle/Fast` coverage row to mark Rust covered.
- [x] Note that bucket thresholds are now centralized. Corrected on 2026-05-29: those thresholds are provisional Rust input facts; Melee `x28`, `x2C`, and `x30` now map to walk velocity ratio/taper fields instead.
- [x] Remove the walk split from the open priority add list.

## Task 4: Verify Slice

- [x] Run `cargo fmt --check`.
- [x] Run `cargo test -p mole_core`.
- [x] Run `cargo test --workspace`.
- [x] Run `git -C "D:\Mole Game\First_Game" diff --check`.
- [x] Record that `python -m pytest` is still blocked by missing `pygame` if it remains true.
