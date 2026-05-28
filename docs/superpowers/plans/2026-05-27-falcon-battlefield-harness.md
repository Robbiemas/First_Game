# Falcon Battlefield Harness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the playable Python prototype an explicit Captain Falcon reference harness on a Melee-unit Battlefield test stage.

**Architecture:** Keep Pygame as the live playtest shell, but move stage geometry into a data profile that is expressed in Melee world units and converted once into the current pixel coordinate system. Keep `DolphinMole` as the current sprite/animation identity, but label its mechanics as the Captain Falcon reference profile so test feedback is anchored to a known character.

**Tech Stack:** Python 3.10, Pygame, existing `pytest` tests, current Rust rollback scaffolding for future parity.

**Status Note:** Paused after the project direction moved to UCF 0.84 as the hard controller baseline. Resume this harness only after UCF-native input facts are available from the Rust WUP path.

---

### Task 1: Stage Profile Data Boundary

**Files:**
- Create: `stage_profiles.py`
- Modify: `stages.py`
- Test: `tests/test_reference_harness.py`

- [ ] **Step 1: Write tests for Melee-unit Battlefield conversion**

Verify that `Stage("first")` aliases the Battlefield test profile, that platform geometry is converted from Melee units using the same `6` scale as the current Falcon movement multiplier, and that blast bounds follow the same origin transform.

- [ ] **Step 2: Add the stage profile**

Define a compact `StageProfile` and `PlatformProfile` with Battlefield data from libmelee-compatible public stage constants: blast zones, main-floor ground edge, side platforms, and top platform. Convert from Melee coordinates `(x right, y up)` to Pygame coordinates `(x right, y down)`.

- [ ] **Step 3: Wire the current stage through the profile**

Keep `Stage("first")` as a compatibility alias so launchers and tests keep working, but expose `Stage("battlefield")` as the actual profile name.

### Task 2: Explicit Falcon Reference Profile

**Files:**
- Modify: `Characters.py`
- Test: `tests/test_reference_harness.py`
- Modify: `docs/research/melee-input-state-reference.md`

- [ ] **Step 1: Write tests that identify the playable test character as Falcon-backed**

The test character should report that `DolphinMole` uses Captain Falcon as its reference mechanics profile while preserving the current sprite identity.

- [ ] **Step 2: Add profile metadata to the character**

Set `referenceCharacter`, `referenceGame`, and `mechanicsProfile` when `dolphinmole()` applies the Falcon values. This is metadata only; no movement tuning belongs in this step.

### Task 3: Verification And QA Loop

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Document what to test**

Add a short Falcon harness QA checklist focused on ground movement, shield into movement, air dodge/waveland, and platform slide-off.

- [ ] **Step 2: Run tests**

Run `pytest` for the Python suite and `cargo test --workspace` for the Rust scaffolding. If the live session is open, let its watcher restart the game; otherwise the user can launch `execs\Live Test Session.cmd`.
