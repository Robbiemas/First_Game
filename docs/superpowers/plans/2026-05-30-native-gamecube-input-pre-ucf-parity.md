# Native GameCube Input Pre-UCF Parity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the WUP adapter mirror native GameCube/Melee pad preprocessing before optional UCF amendments.

**Architecture:** WUP keeps raw adapter bytes as the captured hardware packet, then derives a source-shaped `PADRead`/HSD pad sample at the adapter boundary. The Rust core continues to consume vanilla Melee-shaped input snapshots and does not learn about WUP, SDK origin details, or UCF.

**Tech Stack:** Rust crates `mole_input`, `mole_runtime`, Melee decomp `extern/dolphin/src/dolphin/pad`, and HSD pad code in `src/sysdolphin/baselib/controller.c`.

---

### Task 1: Lock Native Pre-UCF Input Behavior

**Files:**
- Modify: `crates/mole_input/tests/input_contract.rs`
- Modify: `crates/mole_runtime/tests/runtime_contract.rs`

- [x] Add tests proving origin subtraction does not apply the SDK `PADClamp` trigger deadzone before Melee input facts.
- [x] Add tests proving WUP/native preprocessing clamps stick vectors to the HSD radius before UCF or core input.
- [x] Add tests proving the UCF toggle only controls UCF amendments, not native origin/HSD preprocessing.

### Task 2: Implement Adapter-Owned Native Pad Preprocessing

**Files:**
- Modify: `crates/mole_input/src/lib.rs`
- Modify: `crates/mole_runtime/src/wup_input.rs`

- [x] Add a reusable source-shaped native GameCube preprocessing helper in `mole_input`.
- [x] Use `PADRead`-style origin subtraction for sticks and triggers.
- [x] Use Melee HSD-style circular stick clamp with radius 127 and no SDK trigger deadzone.
- [x] Run optional UCF after native preprocessing, while preserving raw origin-subtracted stick samples for UCF source checks.

### Task 3: Document the Boundary

**Files:**
- Modify: `docs/research/melee-input-state-reference.md`
- Modify: `docs/architecture/native-rust-rollback-architecture.md`

- [x] Record the source order: WUP raw bytes, PADRead origin subtraction, HSD clamp/scale, optional UCF, vanilla Melee core facts.
- [x] Note that SDK `PADClamp` is not the Melee fighter input path.

### Task 4: Verify

- [x] Run focused input/runtime tests.
- [x] Run `cargo fmt --check`.
- [x] Run `cargo test --workspace --all-features --jobs 1`.
- [x] Run `cargo clippy --workspace --all-targets --all-features --jobs 1 -- -D warnings`.
