# Rust Core Walk Buckets And Input Facts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the Rust core own explicit `Wait -> WalkSlow/WalkMiddle/WalkFast` transitions from rollback-owned input facts.

**Architecture:** `mole_core` remains deterministic and authoritative. The input snapshot derives a walk bucket fact from current stick magnitude, and the state-local grounded transition code chooses explicit walk motion states while preserving existing walk physics and priority. Pygame remains untouched.

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
- Modify: `crates/mole_core/src/lib.rs`
- Modify: `crates/mole_core/src/state.rs`
- Modify: `crates/mole_core/src/sim.rs`
- Test: `crates/mole_core/tests/core_contract.rs`
- Update: `docs/research/mole-state-coverage-comparison.md`

## Task 1: Add Walk Bucket Input Fact

- [x] Add `WalkSpeedBucket::{None, Slow, Middle, Fast}` to `crates/mole_core/src/input.rs`.
- [x] Add `walk_speed_bucket: WalkSpeedBucket` to `MeleeInputFacts`.
- [x] Derive the bucket from current `lstick.0` inside `MeleeInputSnapshot::facts`.
- [x] Export `WalkSpeedBucket` from `crates/mole_core/src/lib.rs`.
- [x] Add a failing contract test named `melee_input_facts_classify_walk_speed_bucket_from_current_stick`.
- [x] Verify RED with `cargo test -p mole_core melee_input_facts_classify_walk_speed_bucket_from_current_stick`.
- [ ] Implement the minimal helper:

```rust
fn walk_speed_bucket(stick_x: i8, walk_threshold: i8) -> WalkSpeedBucket {
    let magnitude = stick_x.saturating_abs();
    if magnitude < walk_threshold {
        WalkSpeedBucket::None
    } else if magnitude < 50 {
        WalkSpeedBucket::Slow
    } else if magnitude < 90 {
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

## Task 3: Update Coverage Docs

- [x] Update the Rust state list in `docs/research/mole-state-coverage-comparison.md` to include `WalkSlow`, `WalkMiddle`, and `WalkFast`.
- [x] Update the `WalkSlow/Middle/Fast` coverage row to mark Rust covered.
- [x] Note that bucket thresholds are provisional until exact Melee `x28`, `x2C`, and `x30` data is extracted.
- [x] Remove the walk split from the open priority add list.

## Task 4: Verify Slice

- [x] Run `cargo fmt --check`.
- [x] Run `cargo test -p mole_core`.
- [x] Run `cargo test --workspace`.
- [x] Run `git -C "D:\Mole Game\First_Game" diff --check`.
- [x] Record that `python -m pytest` is still blocked by missing `pygame` if it remains true.
