# Native Rust Rollback Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move Mole from a Pygame-authoritative prototype to a native Rust deterministic rollback architecture with SDL3 runtime and peer-to-peer networking.

**Architecture:** Rust owns authoritative simulation, input interpretation, rollback, replay, and transport contracts. SDL3 owns the native shell. Pygame remains only a temporary reference/harness until the Rust runtime is playable.

**Tech Stack:** Rust, Cargo workspace, SDL3, native WUP-028/WinUSB input, fixed 60 Hz deterministic simulation, custom GGRS-shaped rollback, direct UDP, later Supabase signaling and WebRTC DataChannel.

---

## Reference Docs

Read these before implementing:

- `docs/architecture/native-rust-rollback-architecture.md`
- `docs/research/melee-input-state-reference.md`
- `docs/research/melee-common-state-inventory.md`
- `docs/research/mole-state-coverage-comparison.md`
- `docs/research/pygame-movement-logic-pass.md`
- `docs/superpowers/specs/2026-05-27-rust-rollback-core-design.md`

## Working Rules

- Do not revert unrelated user changes.
- Do not delete or clean generated directories unless the user explicitly asks.
- Work against `https://github.com/Robbiemas/First_Game` on branch `handoff/rust-rollback-architecture`.
- Use `gh` and normal `git` CLI commands for repository operations.
- Do not use the Codex GitHub plugin/connector tools unless the user explicitly says they are working again.
- Prefer Rust core changes over Pygame hotfixes.
- Keep `mole_core` free from SDL, sockets, filesystem, wall-clock time, and IO.
- Use tests to lock mechanics before tuning feel.
- Keep each milestone independently runnable and testable.

## Baseline Commands

Run from `D:\Mole Game\First_Game`.

```powershell
cargo test --workspace
python -m pytest
```

Expected for a healthy baseline: Rust and Python tests pass. If either command
fails before new work begins, record the failure in the task notes and avoid
mixing baseline repair with feature implementation unless the failure blocks the
task.

## File Structure Target

Existing or target crates:

- `crates/mole_core`: deterministic simulation and motion states.
- `crates/mole_input`: native GameCube input and UCF-native preprocessing.
- `crates/mole_rollback`: snapshots, prediction, and resimulation.
- `crates/mole_replay`: replay serialization and checksum validation.
- `crates/mole_transport`: loopback, UDP, later WebRTC.
- `crates/mole_runtime`: SDL3 native runtime shell.
- `tools`: state graph viewer and temporary Python tools.
- `execs`: human-facing launchers.

If `mole_input` does not exist yet, create it as a focused crate instead of
letting input preprocessing sprawl through runtime or core code.

The Pygame prototype is no longer allowed to define authoritative mechanics,
but its art and visual data remain valuable migration inputs. Preserve the
dolphin mole sprites, state animation frames, visual primitives, map/background
assets, and related reference timing for later Rust runtime rendering work.
Preserve and repurpose the side-by-side Melee/Mole state graph viewer as a
visual tuning/reference tool for Rust-owned state transitions. Also preserve the
native WUP-028 input path and UCF-style preprocessing as first-class Rust input
infrastructure, not as legacy Pygame mechanics.

## Phase 0: Handoff And Baseline

### Task 0.1: Verify Workspace Status

**Files:**

- Read: all docs listed above.
- Read: `Cargo.toml`
- Read: `crates/*/Cargo.toml`

- [x] Run `git status --short`.
- [x] Record which files are already modified: none before Phase 0 plan updates.
- [x] Do not revert existing changes.
- [x] Run `cargo test --workspace`.
- [x] Run `python -m pytest` through the project `.venv` after global Python reported missing project dependencies.
- [x] If live game processes are running and block builds, close only project-local processes whose command line includes `D:\Mole Game\First_Game`; no blocking project-local game processes were running.

### Task 0.2: Confirm Launch Surface

**Files:**

- Read: `execs/README.md`
- Read: `execs/Live Test Session.cmd`
- Read: `tools/live_test_session.py`

- [x] Confirm how the user launches the current test session: `execs/Live Test Session.cmd` uses the project `.venv` and `tools/live_test_session.py`.
- [x] Confirm the current launcher does not leave background helpers alive after window close: `tools/live_test_session.py` uses an instance lock and stops the game and state graph helper process tree on exit.
- [x] Keep launcher changes separate from mechanics changes.

## Phase 1: Rust Core Movement Authority

### Task 1.1: Split Walk State Identity

**Files:**

- Modify: `crates/mole_core/src/state.rs`
- Modify: `crates/mole_core/src/sim.rs`
- Test: `crates/mole_core/tests/core_contract.rs`
- Update: `docs/research/mole-state-coverage-comparison.md`

- [x] Add failing tests for `Wait -> WalkSlow`, `Wait -> WalkMiddle`, and `Wait -> WalkFast`.
- [x] Tests should assert motion state, facing, state frame, and velocity target behavior.
- [x] Implement explicit `WalkSlow`, `WalkMiddle`, and `WalkFast` states or explicit state metadata if the codebase has already moved that way.
- [x] Keep walk acceleration/target-speed logic data-driven by character attributes.
- [x] Run `cargo test -p mole_core`.
- [x] Update the state coverage doc to mark the walk split as Rust-covered.

### Task 1.2: Keep State-Local Transition Order

**Files:**

- Modify: `crates/mole_core/src/sim.rs`
- Test: `crates/mole_core/tests/core_contract.rs`

- [x] Add tests proving a walk frame checks higher-priority transitions before continuing walk.
- [x] Cover dash-from-walk only when the rollback-owned dash fact is fresh.
- [x] Cover soft opposite stick exiting walk without flipping facing.
- [x] Ensure no generic action resolver can overwrite the state later in the same frame.
- [x] Run `cargo test -p mole_core`.

### Task 1.3: General Landing State

**Files:**

- Modify: `crates/mole_core/src/state.rs`
- Modify: `crates/mole_core/src/sim.rs`
- Test: `crates/mole_core/tests/core_contract.rs`
- Update: `docs/research/mole-state-coverage-comparison.md`

- [x] Add `Landing` as a general landing state distinct from `LandingFallSpecial`.
- [x] Add tests for ordinary airborne landing entering `Landing`.
- [x] Add tests for `FallSpecial` collision entering `LandingFallSpecial`.
- [x] Ensure held shield does not automatically enter guard during landing lag unless the state rules allow it.
- [x] Run `cargo test -p mole_core`.

## Phase 2: GameCube-First Input Crate

### Task 2.1: Establish `mole_input`

**Files:**

- Create or modify: `crates/mole_input/Cargo.toml`
- Create or modify: `crates/mole_input/src/lib.rs`
- Modify: root `Cargo.toml`
- Test: `crates/mole_input/tests/input_contract.rs`

- [x] Add `mole_input` to the Cargo workspace if it does not exist.
- [x] Define `GameCubePadStatus` with raw main stick, C-stick, L/R analog bytes, and button bits.
- [x] Define a deterministic `InputOrigin` captured from stable initial samples.
- [x] Define a packed `PlayerInput` output compatible with rollback.
- [x] Add tests for neutral, max left/right/up/down, C-stick, D-pad, L/R analog, and L/R digital bottom-out.
- [x] Run `cargo test -p mole_input`.

### Task 2.2: Implement Trigger Deadzone As Input Mapping

**Files:**

- Modify: `crates/mole_input/src/lib.rs`
- Test: `crates/mole_input/tests/input_contract.rs`

- [x] Add tests for analog trigger values below the default deadzone mapping to zero pressure.
- [x] Add tests for values above deadzone remapping continuously to the full range.
- [x] Add tests proving L and R remain independent.
- [x] Add tests proving digital bottom-out remains separate from analog pressure.
- [x] Run `cargo test -p mole_input`.

### Task 2.3: Implement UCF-Native Facts

**Files:**

- Modify: `crates/mole_input/src/lib.rs`
- Test: `crates/mole_input/tests/ucf_contract.rs`
- Update: `docs/research/melee-input-state-reference.md`

- [x] Add tests for dashback correction facts.
- [x] Add tests for shield-drop relevant stick facts without turning them into game-state decisions.
- [x] Confirm current local UCF research covers cardinal cleanup, dashback, and shield-drop intent; defer separate snapback-resilient facts until source coverage exists.
- [x] Ensure outputs are facts consumed by the core, not direct state commands.
- [x] Run `cargo test -p mole_input`.

## Phase 3: Runtime Consumes Rust Snapshots

### Task 3.1: Define Runtime Snapshot Boundary

**Files:**

- Modify: `crates/mole_core/src/state.rs`
- Modify: `crates/mole_runtime/src/main.rs`
- Test: `crates/mole_core/tests/core_contract.rs`

- [x] Add or stabilize a read-only world snapshot type for rendering.
- [x] Ensure snapshot contains position, facing, motion state, state frame, animation frame, and debug input facts.
- [x] Ensure snapshot cannot mutate core state.
- [x] Run `cargo test -p mole_core`.

### Task 3.2: SDL3 Local Harness

**Files:**

- Modify: `crates/mole_runtime/Cargo.toml`
- Modify: `crates/mole_runtime/src/main.rs`
- Create or modify: `execs/Run Native Rust Game.cmd`
- Update: `execs/README.md`

- [x] Add SDL3 dependency behind a runtime feature if not already present.
- [x] Open an SDL3 window.
- [x] Run a fixed 60 Hz simulation loop.
- [x] Draw a simple deterministic test character from the Rust snapshot.
- [x] Poll keyboard and generic SDL gamepad as fallback input.
- [x] Keep WUP native input path separate and first-class.
- [x] Confirm closing the window exits the process.
- [x] Run `cargo run -p mole_runtime`.

## Phase 4: Replay And Checksum

### Task 4.1: Replay Contract

**Files:**

- Modify: `crates/mole_replay/src/lib.rs`
- Test: `crates/mole_replay/tests/replay_contract.rs`
- Modify: `crates/mole_core/src/state.rs`

- [x] Add tests that record initial state, input frames, and checksums.
- [x] Add tests that replay the recorded inputs and produce the same final checksum.
- [x] Add tests that a changed input produces a checksum mismatch.
- [x] Run `cargo test -p mole_replay`.

### Task 4.2: Human QA Replay Capture

**Files:**

- Modify: `crates/mole_runtime/src/main.rs`
- Create or modify: `execs/Record Native Replay.cmd`
- Update: `execs/README.md`

- [x] Add a debug mode that records local input frames and periodic checksums.
- [x] Save replay files under a clear project-local debug/replay directory.
- [x] Never make replay capture required for normal play.
- [x] Confirm a captured replay can be played back by tests or a tool.

## Phase 5: Offline Rollback

### Task 5.1: Snapshot Ring

**Files:**

- Modify: `crates/mole_rollback/src/lib.rs`
- Test: `crates/mole_rollback/tests/rollback_contract.rs`

- [x] Add tests for storing snapshots by frame.
- [x] Add tests for restoring an old snapshot.
- [x] Add tests for ring wraparound.
- [x] Run `cargo test -p mole_rollback`.

### Task 5.2: Prediction And Resimulation

**Files:**

- Modify: `crates/mole_rollback/src/lib.rs`
- Test: `crates/mole_rollback/tests/rollback_contract.rs`

- [x] Add tests where remote input for frame N is missing and predicted.
- [x] Add tests where the confirmed frame N input matches prediction.
- [x] Add tests where confirmed frame N differs, causing restore and resimulation.
- [x] Assert final checksum matches the no-delay authoritative path.
- [x] Run `cargo test -p mole_rollback`.

## Phase 6: Direct UDP Transport

### Task 6.1: Transport Trait

**Files:**

- Modify: `crates/mole_transport/src/lib.rs`
- Test: `crates/mole_transport/tests/transport_contract.rs`

- [x] Define a transport boundary that can send and receive versioned packets.
- [x] Implement loopback transport for deterministic tests.
- [x] Add tests for packet ordering, duplicate input frames, and dropped packets.
- [x] Run `cargo test -p mole_transport`.

### Task 6.2: UDP Backend

**Files:**

- Modify: `crates/mole_transport/src/lib.rs`
- Test: `crates/mole_transport/tests/udp_contract.rs`
- Modify: `crates/mole_runtime/src/main.rs`

- [x] Implement nonblocking UDP send/receive.
- [x] Add direct IP/port configuration for development.
- [x] Exchange input frames and checksums.
- [x] Print UDP packet stats in headless runtime debug output.
- [x] Show UDP packet stats in SDL/debug overlay.
- [x] Add frame-based RTT/latency probes in transport packet timing metadata.
- [x] Run two local instances on different ports and confirm input exchange.

## Phase 7: Supabase Signaling

### Task 7.1: Signaling Boundary

**Files:**

- Create: `crates/mole_signaling/Cargo.toml`
- Create: `crates/mole_signaling/src/lib.rs`
- Modify: root `Cargo.toml`
- Test: `crates/mole_signaling/tests/signaling_contract.rs`

- [x] Define signaling messages for room create, room join, offer, answer, ICE candidate, and direct endpoint exchange.
- [x] Keep signaling messages separate from gameplay packets.
- [x] Add tests that serialize and validate signaling messages.
- [x] Run `cargo test -p mole_signaling`.

### Task 7.2: Supabase Prototype

**Files:**

- Modify: `crates/mole_signaling/src/lib.rs`
- Create: `docs/architecture/supabase-signaling-notes.md`

- [x] Use Supabase only for presence, matchmaking, room codes, and setup message exchange.
- [x] Document free-tier constraints and failure modes.
- [x] Keep direct IP/UDP available without Supabase.
- [x] Do not send 60 Hz gameplay input through Supabase.

## Phase 8: WebRTC Transport

### Task 8.1: WebRTC Backend Spike

**Files:**

- Modify: `crates/mole_transport/Cargo.toml`
- Modify: `crates/mole_transport/src/lib.rs`
- Test: `crates/mole_transport/tests/webrtc_contract.rs`
- Update: `docs/architecture/native-rust-rollback-architecture.md`

- [x] Add WebRTC DataChannel as an optional transport backend.
- [x] Exchange setup data through the signaling boundary.
- [x] Run the same rollback transport tests through WebRTC where practical.
- [x] Compare latency/jitter against direct UDP: `TransportTimingComparison` keeps UDP preferred while the WebRTC DataChannel backend has no real runtime timing samples yet.
- [x] Keep UDP as the baseline path.

## Phase 9: Pygame Retirement

### Task 9.1: Mark Pygame As Legacy

**Files:**

- Update: `README.md`
- Update: `execs/README.md`
- Update: `docs/research/pygame-movement-logic-pass.md`

- [x] Document Pygame as historical prototype and temporary QA harness.
- [x] Point normal development to the Rust runtime.
- [x] Keep old launchers available until native Rust reaches feature parity.

### Task 9.2: Remove Duplicate Mechanics From Live Testing

**Files:**

- Modify only after Rust runtime is playable:
  - `Characters.py`
  - `ChooseAction.py`
  - `RealMainFile.py`

- [x] Stop adding new movement mechanics to Pygame; Phase 9.2 only labels the remaining harness.
- [x] If Pygame remains open for QA, make it render Rust snapshots or clearly mark it as legacy: the legacy window caption marks Rust runtime authority.
- [x] Do not allow Pygame movement behavior to override Rust core state; Pygame remains a standalone legacy harness and does not feed authoritative Rust snapshots.

## Completion Definition

The migration is successful when:

- The native Rust runtime launches and closes cleanly.
- Native WUP GameCube input works without an external gamepad mapper.
- Movement state transitions are Rust-owned.
- Replays reproduce checksums.
- Offline rollback corrects mispredictions.
- Direct UDP 1v1 can exchange input and run rollback.
- Supabase, if present, is used only for setup/signaling.
- Pygame is no longer the mechanics authority.

## Recommended First Execution Chunk

Start with Phase 1 and Phase 2 only:

1. Walk state split in Rust.
2. State-local transition tests.
3. `mole_input` crate boundary if not already present.
4. Trigger and UCF-native input contracts.

Do not begin SDL3 runtime or networking until the Rust core can drive the first
movement slice deterministically.
