# Rust Rollback Core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first Rust architecture pass for a deterministic 60 Hz rollback-ready platform-fighter core.

**Architecture:** Add a Rust workspace beside the legacy Python prototype. Keep simulation, rollback, replay, transport, and runtime boundaries in separate crates so the SDL3 host remains thin and the core stays deterministic.

**Tech Stack:** Rust stable, Cargo workspace, standard-library tests first, SDL3 planned as the runtime host dependency after the pure core passes compile and test.

**Input Direction Update:** Native GameCube controller state is now the canonical input shape for the forward engine path. The WUP-028 adapter should produce `GameCubePadStatus` first, preserving raw stick bytes, C-stick bytes, trigger bytes, and button bits. `PlayerInput`, SDL-style `PhysicalInput`, and the current Pygame bridge are downstream compatibility views.

---

## File Structure

- Create: `Cargo.toml` for the workspace.
- Create: `rust-toolchain.toml` to pin stable Rust.
- Modify: `.gitignore` to keep Rust build outputs untracked.
- Modify: `README.md` to document the legacy launcher and new Rust path.
- Delete: `tests/test_native_gamecube.py` because it targets an abandoned Python native-controller experiment and fails without its unfinished module.
- Create: `crates/mole_core/src/lib.rs` exporting deterministic core modules.
- Create: `crates/mole_core/src/time.rs` with `Frame`, `TICK_RATE_HZ`, and fixed-step constants.
- Create: `crates/mole_core/src/input.rs` with packed `PlayerInput`.
- Create: `crates/mole_core/src/state.rs` with deterministic `World`, `PlayerState`, `Vec2`, and checksums.
- Create: `crates/mole_core/src/sim.rs` with `step_world`.
- Create: `crates/mole_core/tests/core_contract.rs` for the core contract tests.
- Create: `crates/mole_rollback/src/lib.rs` with rollback session types.
- Create: `crates/mole_rollback/tests/rollback_contract.rs` for snapshot and resimulation tests.
- Create: `crates/mole_replay/src/lib.rs` with replay logging and checksum validation.
- Create: `crates/mole_replay/tests/replay_contract.rs` for replay tests.
- Create: `crates/mole_transport/src/lib.rs` with transport trait, input packet, and loopback transport.
- Create: `crates/mole_transport/tests/transport_contract.rs` for loopback tests.
- Create: `crates/mole_runtime/src/lib.rs` with fixed-step accumulator logic.
- Create: `crates/mole_runtime/src/main.rs` with a minimal command-line runtime smoke target.
- Create: `crates/mole_runtime/tests/runtime_contract.rs` for accumulator tests.

## Task 1: Workspace And Documentation

**Files:**
- Create: `Cargo.toml`
- Create: `rust-toolchain.toml`
- Modify: `.gitignore`
- Modify: `README.md`
- Delete: `tests/test_native_gamecube.py`

- [ ] **Step 1: Write workspace manifests**

```toml
[workspace]
members = [
  "crates/mole_core",
  "crates/mole_rollback",
  "crates/mole_replay",
  "crates/mole_transport",
  "crates/mole_runtime",
]
resolver = "2"

[workspace.package]
edition = "2021"
license = "MIT"
```

- [ ] **Step 2: Document how to run the Rust path**

Add README sections for `cargo test --workspace`, `cargo run -p mole_runtime`, 60 Hz core rules, and the SDL3 runtime direction.

- [ ] **Step 3: Remove abandoned Python native-controller test**

Delete `tests/test_native_gamecube.py` so the legacy Python smoke tests only cover implemented Python behavior.

## Task 2: Deterministic 60 Hz Core

**Files:**
- Create: `crates/mole_core/Cargo.toml`
- Create: `crates/mole_core/src/lib.rs`
- Create: `crates/mole_core/src/time.rs`
- Create: `crates/mole_core/src/input.rs`
- Create: `crates/mole_core/src/state.rs`
- Create: `crates/mole_core/src/sim.rs`
- Test: `crates/mole_core/tests/core_contract.rs`

- [ ] **Step 1: Write failing core tests**

```rust
use mole_core::{step_world, Frame, PlayerInput, World, TICK_RATE_HZ};

#[test]
fn simulation_rate_is_sixty_hertz() {
    assert_eq!(TICK_RATE_HZ, 60);
}

#[test]
fn packed_input_round_trips_buttons_and_axes() {
    let input = PlayerInput::neutral().with_left_stick(80, -32).with_attack(true).with_jump(true);
    assert_eq!(PlayerInput::from_bits(input.bits()), input);
}

#[test]
fn same_start_and_inputs_produce_same_checksum() {
    let mut a = World::for_two_players();
    let mut b = World::for_two_players();
    let inputs = [PlayerInput::neutral().with_left_stick(127, 0), PlayerInput::neutral()];
    for frame in 0..120 {
        step_world(&mut a, Frame(frame), &inputs);
        step_world(&mut b, Frame(frame), &inputs);
    }
    assert_eq!(a.checksum(), b.checksum());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p mole_core`
Expected: fail because `mole_core` APIs do not exist.

- [ ] **Step 3: Implement minimal deterministic core**

Implement integer movement, gravity, jump, attack flag latching, and stable checksums with no IO or wall-clock reads.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p mole_core`
Expected: all `mole_core` tests pass.

## Task 3: Rollback Snapshots And Resimulation

**Files:**
- Create: `crates/mole_rollback/Cargo.toml`
- Create: `crates/mole_rollback/src/lib.rs`
- Test: `crates/mole_rollback/tests/rollback_contract.rs`

- [ ] **Step 1: Write failing rollback tests**

```rust
use mole_core::{step_world, Frame, PlayerInput, World};
use mole_rollback::{RollbackSession, SnapshotBuffer};

#[test]
fn snapshot_buffer_restores_by_frame() {
    let mut buffer = SnapshotBuffer::new(8);
    let world = World::for_two_players();
    buffer.save(Frame(7), &world);
    assert_eq!(buffer.load(Frame(7)).unwrap().checksum(), world.checksum());
}

#[test]
fn corrected_input_resimulates_to_different_checksum() {
    let initial = World::for_two_players();
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let corrected = [PlayerInput::neutral().with_left_stick(127, 0), PlayerInput::neutral()];
    let mut session = RollbackSession::new(initial.clone(), 32);
    session.advance(Frame(0), neutral);
    let before = session.world().checksum();
    session.correct_and_resimulate(Frame(0), corrected, Frame(1));
    assert_ne!(session.world().checksum(), before);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p mole_rollback`
Expected: fail because rollback types do not exist.

- [ ] **Step 3: Implement snapshot buffer and session**

Use a ring buffer of `(Frame, World)` snapshots and a frame-indexed input history. Restore the latest snapshot before the corrected frame, then resimulate through the requested current frame.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p mole_rollback`
Expected: all rollback tests pass.

## Task 4: Replay Checksums

**Files:**
- Create: `crates/mole_replay/Cargo.toml`
- Create: `crates/mole_replay/src/lib.rs`
- Test: `crates/mole_replay/tests/replay_contract.rs`

- [ ] **Step 1: Write failing replay tests**

```rust
use mole_core::{Frame, PlayerInput, World};
use mole_replay::{ReplayFrame, ReplayLog};

#[test]
fn replay_log_detects_checksum_mismatch() {
    let mut log = ReplayLog::new(World::for_two_players());
    log.push(ReplayFrame {
        frame: Frame(0),
        inputs: [PlayerInput::neutral(), PlayerInput::neutral()],
        checksum: 1,
    });
    assert!(log.validate().is_err());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p mole_replay`
Expected: fail because replay types do not exist.

- [ ] **Step 3: Implement replay log validation**

Store initial world and replay frames in memory. Re-run frames through `mole_core::step_world` and compare expected checksums to actual checksums.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p mole_replay`
Expected: all replay tests pass.

## Task 5: No-Cost Transport Interface

**Files:**
- Create: `crates/mole_transport/Cargo.toml`
- Create: `crates/mole_transport/src/lib.rs`
- Test: `crates/mole_transport/tests/transport_contract.rs`

- [ ] **Step 1: Write failing transport tests**

```rust
use mole_core::{Frame, PlayerInput};
use mole_transport::{InputPacket, LoopbackTransport, Transport};

#[test]
fn loopback_transport_delivers_input_packets() {
    let mut transport = LoopbackTransport::default();
    let packet = InputPacket { frame: Frame(9), player_index: 0, input: PlayerInput::neutral().with_attack(true) };
    transport.send(packet);
    assert_eq!(transport.try_recv(), Some(packet));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p mole_transport`
Expected: fail because transport types do not exist.

- [ ] **Step 3: Implement trait and loopback transport**

Use a `VecDeque<InputPacket>` to model packet delivery without sockets. Keep the trait synchronous and allocation-light.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p mole_transport`
Expected: all transport tests pass.

## Task 6: Runtime Fixed-Step Shell

**Files:**
- Create: `crates/mole_runtime/Cargo.toml`
- Create: `crates/mole_runtime/src/lib.rs`
- Create: `crates/mole_runtime/src/main.rs`
- Test: `crates/mole_runtime/tests/runtime_contract.rs`

- [ ] **Step 1: Write failing runtime tests**

```rust
use mole_runtime::FixedStepClock;

#[test]
fn fixed_step_clock_emits_one_tick_for_one_sixtieth_second() {
    let mut clock = FixedStepClock::default();
    assert_eq!(clock.add_elapsed_nanos(1_000_000_000 / 60), 1);
}

#[test]
fn fixed_step_clock_keeps_remainder_for_next_frame() {
    let mut clock = FixedStepClock::default();
    assert_eq!(clock.add_elapsed_nanos(8_000_000), 0);
    assert_eq!(clock.add_elapsed_nanos(9_000_000), 1);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p mole_runtime`
Expected: fail because runtime clock does not exist.

- [ ] **Step 3: Implement fixed-step shell and smoke binary**

The binary runs a short deterministic simulation and prints the final frame and checksum. SDL3 input/render code attaches here next, without touching the core API.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p mole_runtime`
Expected: all runtime tests pass.

## Task 7: Whole-Workspace Verification

**Files:**
- All new Rust crates.
- Existing Python launcher tests.

- [ ] **Step 1: Format Rust**

Run: `cargo fmt --all -- --check`
Expected: no formatting diffs.

- [ ] **Step 2: Test Rust**

Run: `cargo test --workspace`
Expected: all Rust tests pass.

- [ ] **Step 3: Test legacy Python smoke coverage**

Run: `.\.venv\Scripts\python -m pytest -q`
Expected: implemented Python smoke tests pass.

- [ ] **Step 4: Smoke-run the new runtime**

Run: `cargo run -p mole_runtime -- --frames 120`
Expected: command prints `final_frame=120` and a checksum.

## Follow-Up Task: Melee-Style GameCube Interpreter

**Files:**
- Modify: `crates/mole_core/src/input.rs`
- Test: `crates/mole_core/tests/core_contract.rs`
- Modify: `crates/mole_runtime/src/readout.rs`

- [ ] **Step 1: Write failing interpreter tests**

Cover previous/current `GameCubePadStatus` pairs for jump press, dash threshold, tilt threshold, smash turn threshold, C-stick direction, analog shield pressure, digital trigger click, and D-pad state.

- [ ] **Step 2: Implement a tunable interpreter**

Add a small deterministic interpreter that consumes raw GameCube pad frames and produces semantic facts for the core. Keep thresholds centralized so Melee-like defaults can be tuned without touching game rules.

- [ ] **Step 3: Expose interpreter debug output**

Extend the WUP monitor/readout path so it can show raw bytes, centered values, and interpreted facts side by side.

## Self-Review

- Spec coverage: the plan maps every design requirement to one of the seven tasks.
- Placeholder scan: there are no `TBD`, `TODO`, or undefined future steps.
- Type consistency: crate names, struct names, and method signatures match across task tests and implementation notes.
