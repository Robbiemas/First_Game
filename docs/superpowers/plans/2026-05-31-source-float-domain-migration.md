# Source Float Domain Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move Melee-owned fighter input and movement physics math from Rust milli/fixed approximations to source-shaped `f32` domains while preserving deterministic rollback snapshots through exact bit hashing.

**Architecture:** Raw adapter data remains raw bytes and HSD-clamped/scaled stick values in `mole_input`; UCF remains outside `mole_core`. The native input layer mirrors Melee's HSD pad path (`clamp_stickMax = 80`, `scale_stick = 80`) before encoding the normalized result into the current compact signed-127 rollback axis. `mole_core` consumes vanilla Melee-shaped fighter input, derives normalized stick floats from that compact bridge while it exists, stores source-owned fighter/common physics values as `f32`, and hashes/snapshots their exact IEEE-754 bit patterns with `to_bits()`.

**Tech Stack:** Rust `f32`, existing `mole_core` deterministic world/checksum model, local Melee decomp under `.research/doldecomp-melee`, extracted PlCo/Falcon DAT resources, existing Slippi replay diagnostics and parity tooling.

---

## Source Evidence

- `D:\Mole Game\.research\doldecomp-melee\src\melee\gm\gmmain.c`: game startup sets `clamp_stickMax = 80`, `clamp_stickMin = 0`, `clamp_stickShift = 1`, and `scale_stick = 80`.
- `D:\Mole Game\.research\doldecomp-melee\src\sysdolphin\baselib\controller.c`: `HSD_PadClampCheck3` clamps signed stick vectors to the configured max, and `HSD_PadScale` computes `nml_stickX = (f32) stickX / (f32) scale_stick`.
- `D:\Mole Game\.research\doldecomp-melee\src\melee\ft\fighter.c`: `Fighter_Spaghetti_8006AD10` copies `HSD_PadGameStatus[slot].nml_stickX/Y` into `fp->input.lstick.x/y` and zeroes deadzones with float comparisons.
- `D:\Mole Game\.research\doldecomp-melee\src\melee\ft\inlines.h`: `getAccelAndTarget` multiplies `fp->input.lstick.x` directly by `dash_run_acceleration_a` and `dash_run_terminal_velocity`.
- `D:\Mole Game\.research\doldecomp-melee\src\melee\ft\chara\ftCommon\ftCo_Dash.c`, `ftCo_Run.c`, `ftCo_TurnRun.c`, `ftCo_Jump.c`, `ftCo_JumpAerial.c`, and `ftCo_FallSpecial.c` operate on `fp->gr_vel` and `fp->self_vel` as floats.

## Domain Rules

- Raw WUP/native adapter bytes: `u8`, owned by `mole_input`/runtime.
- HSD-clamped stick bytes: signed `i8` in approximately `-80..=80`, owned by the adapter preprocessing layer before UCF.
- Current compact rollback axis: signed `i8` in `-127..=127`, representing the HSD normalized float while the core still stores packed stick bytes.
- Fighter normalized stick: source `f32`, computed by Melee as `hsd_stick_byte as f32 / 80.0`; during the transition Rust reconstructs the equivalent value from the compact signed-127 bridge.
- Source-owned physics attributes and PlCo scalar values: `f32` where the DAT/decomp field is `float`.
- Discrete timers, command vars, action frames, button bits, and state ids: integers.
- Determinism: checksum `f32` with `to_bits()`, and keep replay snapshots exact enough to reconstruct the same `f32` values.

## Migration Order

### Task 1: Add a Fighter Float Domain Without Removing Raw Input

**Files:**
- Modify: `crates/mole_core/src/input.rs`
- Modify: `crates/mole_core/src/state.rs`
- Test: `crates/mole_core/tests/core_contract.rs`

- [x] Add a `FighterStick` or equivalent helper that exposes `lstick_x/y` and `cstick_x/y` as `f32` using `axis as f32 / 127.0`.
- [x] Add tests proving `127 -> 1.0`, `-127 -> -1.0`, and `0 -> 0.0`.
- [x] Keep `PlayerInput` raw packed stick fields as `i8` so rollback input snapshots remain compact and WUP/UCF boundaries stay outside the core.

### Task 2: Convert Dash/Run Accel and Target to Source `f32`

**Files:**
- Modify: `crates/mole_core/src/state.rs`
- Modify: `crates/mole_core/src/sim.rs`
- Test: `crates/mole_core/tests/core_contract.rs`

- [x] Change Falcon profile fields used by `getAccelAndTarget` from milli integers to source `f32`: `dash_run_acceleration_a`, `dash_run_acceleration_b`, `dash_run_terminal_velocity`.
- [x] Add a failing test proving full HSD-clamped left stick `-127` produces target velocity `-2.3` and accel `-0.16`, matching `input.lstick.x == -1.0f`.
- [x] Add a failing test for the Slippi-observed `stick_x == -125` path proving target velocity is `(-125.0 / 127.0) * 2.3` without the old negative-128 denominator.
- [x] Implement `dash_run_accel_and_target` with `f32` math matching `getAccelAndTarget`.
- [x] Keep render/readout compatibility by converting displayed values to the existing visual unit scale only at the boundary.

### Task 3: Hash and Snapshot Float Bits

**Files:**
- Modify: `crates/mole_core/src/state.rs`
- Modify: `crates/mole_runtime/src/readout.rs`
- Test: `crates/mole_core/tests/core_contract.rs`

- [x] Add `mix_f32` using `value.to_bits()`.
- [x] Replace checksum mixing for migrated profile/common/player float fields with `mix_f32`.
- [x] Add a checksum test proving two worlds that differ only by a migrated float field have different checksums.
- [x] Expose migrated grounded source-float values in runtime/debug traces.
- [ ] Expose both source-float values and legacy display-scaled values in debug traces where useful.

### Task 4: Convert Player Movement State Floats

**Files:**
- Modify: `crates/mole_core/src/state.rs`
- Modify: `crates/mole_core/src/sim.rs`
- Modify: `crates/mole_runtime/src/readout.rs`
- Test: `crates/mole_core/tests/core_contract.rs`

- [x] Convert grounded source-owned `PlayerState` motion fields to `f32`: `ground_velocity_x`, `ground_accel_x`, `ground_accel_x2`, dash entry velocity delta, Dash `x0`, and walk animation velocity.
- [ ] Convert `PlayerState` source-owned `position`, public `velocity`, and motion animation rate to `f32`.
- [ ] Keep ECB visualization and stage/image rendering conversions at boundary functions, not in gameplay math.
- [x] Add tests for staged Dash/Run/Walk source-float accumulation and source float expected values around grounded acceleration, jump carry, and checksums.
- [ ] Add source-float tests for remaining public-air `velocity` paths after those fields migrate.
- [ ] Update Slippi comparison tolerance/reporting to show float source units and optional display units.

### Task 5: Convert Common/Profile Movement Scalars

**Files:**
- Modify: `crates/mole_core/src/common_data.rs`
- Modify: `crates/mole_core/src/state.rs`
- Modify: `tools/generate_value_sheets.py`
- Modify: `tools/export_parity_diff_report.py`
- Test: `tests/test_value_sheets.py`
- Test: `tests/test_parity_diff_report.py`

- [x] Convert grounded PlCo float ratios/scalars from `_milli` integers to `f32`: walk ratios/tapers, dash velocity decay, run friction multiplier, high-speed friction multiplier, RunBrake animation pause velocity, and animation velocity scale.
- [x] Convert remaining PlCo float ratios/scalars from `_milli` integers to `f32` where the decomp field is float: EscapeAir force/decay, fall animation blend, pass initial velocity, entry scale, and related non-grounded scalar fields.
- [x] Convert Falcon movement attributes from `_per_tick` milli integers to source `f32` names where decomp values are floats.
- [ ] Preserve discrete frame counts and thresholds as integers.
- [x] Regenerate value sheets and parity reports with source-float formatting for migrated grounded common-data fields.
- [x] Regenerate value sheets and parity reports with float bit/value formatting for remaining migrated fields as later slices land.

Completed slice note:
- The extractor, Rust common/profile structs, tests, value sheets, parity report, and state graph value refs now agree on source `f32` names for exposed Melee float fields. Public position/velocity and some debug display bridges still convert through the existing milli boundary until the next source-position migration slice.

### Task 6: Re-run Replay Oracle and Continue Bottom-Up Parity

**Files:**
- Modify as needed: `crates/mole_runtime/src/slippi_diagnostic.rs`
- Modify as needed: `docs/research/2026-05-31-grounded-locomotion-difference-sheet.md`

- [ ] Run the Slippi match-start trace around P2 dash/kneebend and verify the earliest drift shifts or disappears.
- [ ] If drift remains, inspect the next decomp boundary before changing code.
- [ ] Record the new finding in the grounded locomotion sheet.
- [ ] Do not tune moonwalk/dash dance by feel; use playtest feel only as diagnostic feedback after source-shaped changes.

## Verification

Run before claiming this migration slice complete:

```powershell
cargo fmt
cargo test --workspace
.venv\Scripts\python.exe -m pytest tests\test_value_sheets.py tests\test_state_graph_viewer.py tests\test_launch_inputs.py tests\test_parity_diff_report.py tests\test_generate_falcon_ecb_rust.py tests\test_extract_melee_resources.py tests\test_slippi_replay_tools.py -q
.venv\Scripts\python.exe tools\state_graph_viewer.py --check
cargo run -p mole_cli -- parity --json
cargo run -p mole_cli -- parity snapshot --json
git diff --check
```
