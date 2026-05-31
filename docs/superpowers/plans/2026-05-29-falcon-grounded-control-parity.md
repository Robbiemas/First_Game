# Falcon Grounded Control Parity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring Falcon's grounded stick-box, dash/moonwalk split, and Dash velocity-over-time behavior closer to Melee decomp parity.

**Architecture:** Keep Rust authoritative and deterministic. Model moonwalk as an emergent Dash result from processed stick history, tap timers, live stick acceleration/target velocity, Falcon physics values, and Melee common-data thresholds. Do not add a `Moonwalk` state.

**Tech Stack:** Rust `mole_core`, local Melee decomp reference in `D:\Mole Game\.research\doldecomp-melee`, extracted `PlCo.dat`/Captain Falcon JSON resources, `cargo test`.

**Current checkpoint status:** Tasks 1-4 are implemented in the working tree and covered by focused Rust tests. Tasks 6-7 are the moonwalk-distance parity corrections for non-run Dash completion and full-walk carry. Task 8 is the source-order correction for `ftCo_Dash_Enter`'s staged entry delta, same-frame `Dash_Phys` consumption, next-frame live-stick acceleration, and tap-start early-Dash gate. Full workspace verification has passed; commit/push is deferred until user review.

---

### Task 1: Source Grounded-Control Common Data

**Files:**
- Modify: `D:\Mole Game\First_Game\crates\mole_core\tests\core_contract.rs`
- Modify: `D:\Mole Game\First_Game\crates\mole_core\src\common_data.rs`
- Modify: `D:\Mole Game\First_Game\crates\mole_core\src\state.rs`
- Modify: `D:\Mole Game\First_Game\tools\extract_melee_resources.py`
- Regenerate: `D:\Mole Game\First_Game\resources\melee\extracted\plco_common_data.json`

- [x] **Step 1: Add failing tests for extracted grounded-control defaults**

Add focused tests proving `MeleeCommonData::provisional_mole()` uses Melee extracted values for grounded control:

```rust
assert_eq!(common.main_stick_deadzone_x, 36);
assert_eq!(common.main_stick_deadzone_y, 36);
assert_eq!(common.tap_x_threshold, 32);
assert_eq!(common.tap_y_threshold, 32);
assert_eq!(common.walk_x, 23);
assert_eq!(common.turn_x, -32);
assert_eq!(common.dash_x, 102);
assert_eq!(common.dash_tap_window, 2);
assert_eq!(common.dash_early_action_window, 4);
assert_eq!(common.dash_defensive_action_window, 3);
assert_eq!(common.dash_late_action_window, 20);
assert_eq!(common.dash_velocity_decay_milli, 750);
assert_eq!(common.run_x, 79);
assert_eq!(common.run_turn_run_no_interrupt_frames, 10);
```

- [x] **Step 2: Run the focused tests and confirm they fail on stale provisional values**

Run: `cargo test -p mole_core melee_common_data_provisional -- --nocapture`

- [x] **Step 3: Extract and wire the missing PlCo fields**

Add `main_stick_deadzone_x` from `PlCo+0x00`, `main_stick_deadzone_y` from `PlCo+0x04`, and `dash_velocity_decay_milli` from `PlCo+0x54`. Feed separate X/Y deadzones into `MeleeInputConfig`, and include new fields in deterministic state hashing.

- [x] **Step 4: Update provisional common data to match extracted PlCo values**

Use source values already extracted from local `PlCo.dat`, not hand tuning. Keep defensive/shield thresholds outside this checkpoint unless a test already proves they must move with these fields.

- [x] **Step 5: Run the focused tests**

Run: `cargo test -p mole_core melee_common_data_provisional -- --nocapture`

### Task 2: Preserve Melee Stick Scaling and Tap-Timer Boundaries

**Files:**
- Modify: `D:\Mole Game\First_Game\crates\mole_core\tests\core_contract.rs`
- Modify: `D:\Mole Game\First_Game\crates\mole_core\src\input.rs`
- Modify: `D:\Mole Game\First_Game\crates\mole_core\src\sim.rs`

- [x] **Step 1: Add failing tests for Melee-style stick normalization**

Prove positive full stick uses `127` as full scale and negative full stick uses `128` as full scale, so raw full-left does not overdrive movement beyond full magnitude.

- [x] **Step 2: Add failing golden tests for the Dash boundary**

Cover these sequences:
- Fresh opposite full-stick during Dash enters the opposite turn/dash-back path.
- Opposite stick that ages past `x40` stays in Dash and applies backward velocity.
- Soft outward stick travel crosses walk/run thresholds without accidentally creating Dash.

- [x] **Step 3: Implement the smallest input/scaling fixes**

Use separate X/Y deadzones and Melee asymmetric stick scaling. Keep UCF preprocessing in the input layer and keep all checks deterministic over input snapshots.

- [x] **Step 4: Run the focused tests**

Run: `cargo test -p mole_core stick -- --nocapture`

### Task 3: Add Dash IASA Velocity Decay

**Files:**
- Modify: `D:\Mole Game\First_Game\crates\mole_core\tests\core_contract.rs`
- Modify: `D:\Mole Game\First_Game\crates\mole_core\src\sim.rs`

- [x] **Step 1: Add failing tests for the decomp Dash IASA decay branch**

Prove Dash applies the `gr_vel += -(gr_vel * x54) * ground_friction_multiplier` branch only after Dash IASA transitions are evaluated and only when the Dash IASA path reaches the decomp block.

- [x] **Step 2: Implement the decay branch where Dash IASA currently resolves**

Use `dash_velocity_decay_milli` and stage/floor friction multiplier. Do not replace the existing Dash physics `getAccelAndTarget` path; this branch is additive to the decomp control flow.

- [x] **Step 3: Run focused Dash tests**

Run: `cargo test -p mole_core dash -- --nocapture`

### Task 4: Preserve Source Ground-Velocity Lifecycle Through Turn, Wait, And Dash Entry

**Files:**
- Modify: `D:\Mole Game\First_Game\crates\mole_core\tests\core_contract.rs`
- Modify: `D:\Mole Game\First_Game\crates\mole_core\src\sim.rs`
- Modify: `D:\Mole Game\First_Game\docs\research\melee-input-state-reference.md`
- Modify: `D:\Mole Game\First_Game\docs\research\mole-state-coverage-comparison.md`

- [x] **Step 1: Add failing tests for carried ground velocity**

Cover Dash-to-Turn, Turn-to-Dash, and Dash-to-Wait paths that previously zeroed velocity.
The tests should prove `Turn` applies source-shaped ground friction, Dash entry uses
`ftCo_Dash_Enter`'s initial velocity delta rule, and Wait friction preserves post-dash
slide instead of deleting it.

- [x] **Step 2: Implement the smallest source-backed velocity lifecycle fix**

Keep `velocity.x` as the Rust `gr_vel` equivalent for this slice. Preserve it through
Turn and Wait transitions, apply `ft_80084F3C`-shaped ground traction while those states
idle, and compute Dash entry as initial dash delta rather than an unconditional overwrite.

- [x] **Step 3: Run focused movement tests**

Run:
```powershell
cargo test -p mole_core dash -- --nocapture
cargo test -p mole_core moonwalk -- --nocapture
cargo test -p mole_core turn -- --nocapture
cargo test -p mole_core jump_takeoff -- --nocapture
cargo test -p mole_core air_drift -- --nocapture
```

### Task 5: Verify Falcon Grounded Feel Surface

**Files:**
- Modify as needed: `D:\Mole Game\First_Game\docs\research\melee-input-state-reference.md`
- Modify as needed: `D:\Mole Game\First_Game\docs\research\mole-state-coverage-comparison.md`
- Modify as needed: state graph metadata if Rust state coverage changed

- [x] **Step 1: Update docs with exact extracted thresholds and decomp source paths**

Record the threshold/timer/decay values and explain that moonwalk remains emergent from Dash, not a state.

- [x] **Step 2: Run full verification**

Run:
```powershell
cargo fmt --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
git diff --check
```

- [x] **Step 3: Launch the runtime for user testing**

Start the SDL3 runtime as usual and verify state text/debug readouts still appear for both characters.

### Task 6: Correct Non-Run Dash Completion For Moonwalk Follow-Through

**Files:**
- Modify: `D:\Mole Game\First_Game\crates\mole_core\tests\core_contract.rs`
- Modify: `D:\Mole Game\First_Game\crates\mole_core\src\sim.rs`
- Modify: `D:\Mole Game\First_Game\docs\research\melee-input-state-reference.md`
- Modify: `D:\Mole Game\First_Game\docs\research\mole-state-coverage-comparison.md`
- Modify: `D:\Mole Game\First_Game\docs\state_graphs\melee_reference_graph.json`
- Modify: `D:\Mole Game\First_Game\docs\state_graphs\mole_current_graph.json`

- [x] **Step 1: Add source-shaped tests for aged opposite Dash completion**

Prove a moonwalk-style aged opposite hold exits Falcon Dash to `Wait` with facing
and carried ground velocity preserved, then the next Wait tick handles the held
opposite stick through the ordinary `Turn` path. Also prove neutral after this
Wait handoff applies ground friction rather than snapping to a walk target.

- [x] **Step 2: Remove the direct Dash-to-Walk shortcut**

Keep the same-facing run handoff, but make all other Dash animation completions
return to `Wait`. This follows `ftCo_Dash_Anim`/`ft_8008A2BC` plus the
`fn_800CA5F0` run-only handoff shape instead of inventing a Dash-to-Walk route.

- [x] **Step 3: Update docs and graph metadata**

Record that moonwalk follow-through is `Dash -> Wait -> Turn/Walk` via normal
Wait priority, not a direct `Dash -> WalkFast` transition.

- [x] **Step 4: Run focused and full verification**

Run under the current workspace permissions:

```powershell
cargo test -p mole_core moonwalk -- --nocapture
cargo test -p mole_core dash -- --nocapture
cargo test -p mole_core turn -- --nocapture
cargo fmt --check
git diff --check
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

### Task 8: Mirror Dash Entry Staged Delta Ordering

**Files:**
- Modify: `D:\Mole Game\First_Game\crates\mole_core\tests\core_contract.rs`
- Modify: `D:\Mole Game\First_Game\crates\mole_core\src\state.rs`
- Modify: `D:\Mole Game\First_Game\crates\mole_core\src\sim.rs`
- Modify: `D:\Mole Game\First_Game\docs\research\melee-input-state-reference.md`
- Modify: `D:\Mole Game\First_Game\docs\research\mole-state-coverage-comparison.md`
- Modify: `D:\Mole Game\First_Game\docs\state_graphs\mole_current_graph.json`

- [x] **Step 1: Add regression tests for `mv.co.dash.x0` ordering**

Prove Dash entry stages the initial velocity delta through the transition
frame's position integration, applies it to ground velocity after that
translation, and consumes it before the next input frame can run live-stick
`getAccelAndTarget` acceleration. Add the moonwalk payload trace:
`(+127,0) -> (-101,-45) -> (-101,-45) -> (-128,0)`, matching normalized
`(1.0,0.0) -> (-0.7875,-0.35) -> (-0.7875,-0.35) -> (-1.0,0.0)`. Strengthen
the full-speed walk carry test so it starts from Falcon walk max speed and
verifies the first stick-roll frame applies opposite live-stick influence
immediately after the staged entry delta has been consumed.

- [x] **Step 2: Store the staged delta in rollback state**

Add a deterministic `dash_entry_velocity_delta` field to `PlayerState`, include
it in the checksum, set it from `ftCo_Dash_Enter`'s delta rule, apply it after
the transition frame's position update, and clear it as soon as the player
leaves `Dash`.

- [x] **Step 3: Consume the staged delta before the next live Dash physics tick**

Melee runs `ftCo_Dash_Phys` on the same engine frame as a Wait/Walk/Turn to
Dash transition because IASA changes the motion state before the physics
callback. That same-frame physics callback consumes `mv.co.dash.x0`; the next
input frame must therefore run the ordinary Dash accel/target/friction branch.
Rust mirrors this by staging the delta through translation, applying it to
`velocity.x` after the position update, and clearing it before the next
rollback-owned input snapshot.

- [x] **Step 4: Mirror tap-start early-Dash gating**

Store whether Dash was entered from the normal tap-start path, mirroring
`mv.co.dash.x4`. While the early tap-start Dash branch is active, block fresh
opposite dash-back checks and let live stick physics handle the bottom-gate
moonwalk trace.

- [x] **Step 5: Update docs and graph metadata**

Record that WUP raw bytes, UCF diagnostics, and Melee/HSD-shaped input facts
remain separate layers, and that Rust `Dash` now tracks the staged entry delta
ordering and tap-start early-Dash gate explicitly.

- [x] **Step 6: Run focused and full verification**

Run under the current workspace permissions:

```powershell
cargo test -p mole_core dash_entry_delta_updates_ground_velocity_after_translation_only -- --nocapture
cargo test -p mole_core dash_entry_delta_is_consumed_before_next_dash_physics_tick -- --nocapture
cargo test -p mole_core moonwalk_payload_bottom_gate_trace_stays_dash_and_applies_opposite_influence -- --nocapture
cargo test -p mole_core dash_neutral_stick_applies_dash_friction_before_dash_ends -- --nocapture
cargo test -p mole_core walk_rollout_frame_preserves_full_walk_carry_for_opposite_dash_moonwalk_setup -- --nocapture
cargo test -p mole_core moonwalk -- --nocapture
cargo test -p mole_core dash -- --nocapture
cargo test -p mole_core walk -- --nocapture
cargo fmt --check
git diff --check
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

### Task 7: Preserve Full-Speed Walk Carry Into Opposite Dash Moonwalk Setup

**Files:**
- Modify: `D:\Mole Game\First_Game\crates\mole_core\tests\core_contract.rs`
- Modify: `D:\Mole Game\First_Game\crates\mole_core\src\sim.rs`
- Modify: `D:\Mole Game\First_Game\docs\research\melee-input-state-reference.md`
- Modify: `D:\Mole Game\First_Game\docs\research\mole-state-coverage-comparison.md`
- Modify: `D:\Mole Game\First_Game\docs\state_graphs\melee_reference_graph.json`
- Modify: `D:\Mole Game\First_Game\docs\state_graphs\mole_current_graph.json`

- [x] **Step 1: Add failing tests for full-walk carry through rollout**

Prove a full-speed `WalkFast` carry survives a sampled below-walk rollout frame,
then feeds a fresh opposite dash tap into `Turn` and the next-frame Dash entry.
The expected Dash velocity is `walk_gr_vel - Falcon.initial_dash_speed`, matching
`ftCo_Dash_Enter`'s additive opposing-velocity branch. Continue the trace with a
staged stick roll back toward the carried direction so the opposite fresh tap
ages out before full dash strength; this must stay in Dash and increase
backward moonwalk velocity without adding a `Moonwalk` state.

- [x] **Step 2: Remove the Walk-to-Wait velocity clear**

Model `ft_8008A244`: exiting Walk to Wait changes state/facing timing but does
not clear `gr_vel`. Wait friction can settle the carry later.

- [x] **Step 3: Update docs and graph metadata**

Record that `Walk -> Wait` preserves carried ground velocity and is part of
Captain Falcon's max-length moonwalk setup surface.

- [x] **Step 4: Run focused and full verification**

Run under the current workspace permissions:

```powershell
cargo test -p mole_core walk_rollout_frame_preserves_full_walk_carry_for_opposite_dash_moonwalk_setup -- --nocapture
cargo test -p mole_core walk_state_soft_opposite_stick_exits_to_wait_without_flipping_facing -- --nocapture
cargo test -p mole_core moonwalk -- --nocapture
cargo test -p mole_core dash -- --nocapture
cargo fmt --check
git diff --check
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

### Task 9: Audit Adjacent Velocity Staging Before Claiming Movement Parity

**Files:**
- Modify as needed: `D:\Mole Game\First_Game\docs\research\melee-input-state-reference.md`
- Modify as needed: `D:\Mole Game\First_Game\docs\research\mole-state-coverage-comparison.md`
- Modify as needed: `D:\Mole Game\First_Game\crates\mole_core\tests\core_contract.rs`
- Modify as needed: `D:\Mole Game\First_Game\crates\mole_core\src\sim.rs`

- [x] **Step 1: Classify the Dash fix against Melee staged velocity fields**

Record that the source-backed Dash correction specifically covers
`ftCommon_800804A0`/`xE8_ground_accel_2`, where Dash entry changes `gr_vel`
after the transition frame's translation. Record that ordinary flat-ground
`xE4_ground_accel_1` accel/friction remains equivalent to the current direct
Rust update only while movement and end-of-frame `gr_vel` receive the same
delta.

- [x] **Step 2: Verify other `xE8` users before adding similar movement**

Search the local decomp for `ftCommon_800804A0` before implementing any new
state that uses it. Current source evidence shows Dash and Rebound; Dash is in
this checkpoint, Rebound is not movement-feel priority yet.

- [ ] **Step 3: Audit jump and aerial velocity separately**

Use the decomp `self_vel`/`x74_anim_vel` paths in `ftCo_Jump.c`,
`ftCo_JumpAerial.c`, and `ftCo_EscapeAir.c` before changing jump carry, air
drift, aerial jumps, or air-dodge follow-through. Do not infer these from the
ground `gr_vel`/`xE8` Dash fix.
