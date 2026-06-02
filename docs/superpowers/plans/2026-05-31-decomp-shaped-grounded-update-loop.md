# Decomp-Shaped Grounded Update Loop Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace grounded locomotion shortcuts with a Melee-decomp-shaped update slice for Wait, Walk, Turn, Dash, Run, RunBrake, and TurnRun while keeping the dev tools congruent with the Rust core.

**Architecture:** Keep Rust authoritative and deterministic. Introduce a small ground-frame staging layer that separates Melee-style `anim`, `input/IASA`, `phys`, acceleration commit, position commit, and `coll` phases without rewriting the whole engine. Start with grounded locomotion only; do not tune constants or add non-Melee mechanics.

**Tech Stack:** Rust workspace (`mole_core` first), existing fixed-point milli unit model, existing generated parity sheets and Python state graph viewer.

---

## Source Anchors

- Decomp dash source: `D:\Mole Game\.research\doldecomp-melee\src\melee\ft\chara\ftCommon\ftCo_Dash.c`
- Decomp turn source: `D:\Mole Game\.research\doldecomp-melee\src\melee\ft\chara\ftCommon\ftCo_Turn.c`
- Decomp run source: `D:\Mole Game\.research\doldecomp-melee\src\melee\ft\chara\ftCommon\ftCo_Run.c`
- Decomp run-brake source: `D:\Mole Game\.research\doldecomp-melee\src\melee\ft\chara\ftCommon\ftCo_RunBrake.c`
- Decomp turn-run source: `D:\Mole Game\.research\doldecomp-melee\src\melee\ft\chara\ftCommon\ftCo_TurnRun.c`
- Decomp movement helpers: `D:\Mole Game\.research\doldecomp-melee\src\melee\ft\ftcommon.c`
- Decomp fighter update order: `D:\Mole Game\.research\doldecomp-melee\src\melee\ft\fighter.c`

Important movement-order nuance: `ftCommon_ApplyGroundMovement` derives flat-ground movement from current `gr_vel` plus `xE4_ground_accel_1`, while `fighter.c` separately commits `gr_vel += xE4_ground_accel_1 + xE8_ground_accel_2`. Dash entry uses `ftCommon_800804A0`/`xE8`, so the initial dash delta updates ground velocity for following frames but does not translate position on the transition frame.

## Files

- Modify: `D:\Mole Game\First_Game\crates\mole_core\src\state.rs`
- Modify: `D:\Mole Game\First_Game\crates\mole_core\src\sim.rs`
- Modify: `D:\Mole Game\First_Game\crates\mole_core\src\lib.rs` only if a new module is introduced.
- Test: `D:\Mole Game\First_Game\crates\mole_core\tests\core_contract.rs`
- Modify: `D:\Mole Game\First_Game\docs\research\melee-input-state-reference.md`
- Modify: `D:\Mole Game\First_Game\docs\research\mole-state-coverage-comparison.md`
- Modify: `D:\Mole Game\First_Game\docs\state_graphs\mole_current_graph.json`
- Modify if graph node IDs or launcher UX changes: `D:\Mole Game\First_Game\config\state_graph_layout.json`
- Modify if launcher UX changes: `D:\Mole Game\First_Game\execs\Run SDL3 Runtime.cmd`
- Modify if launcher UX changes: `D:\Mole Game\First_Game\execs\Open State Graphs.cmd`
- Test: `D:\Mole Game\First_Game\tests\test_state_graph_viewer.py`
- Test: `D:\Mole Game\First_Game\tests\test_value_sheets.py`
- Test: `D:\Mole Game\First_Game\tests\test_parity_diff_report.py`

## Task 1: Lock Decomp-Shaped Ground Staging With Red Tests

**Files:**
- Modify: `D:\Mole Game\First_Game\crates\mole_core\tests\core_contract.rs`

- [ ] **Step 1: Add a Dash entry phase-order test**

Add this test near the existing Dash entry tests. It names the source path and protects the distinction between transition-frame translation, `xE8`-style Dash entry delta, and next-frame ordinary Dash physics.

```rust
#[test]
fn dash_entry_uses_source_xe8_staging_before_next_dash_phys() {
    let mut world = World::for_two_players();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(127, 0),
        PlayerInput::neutral(),
    ];

    let start_x = world.players()[0].position.x;

    step_world(&mut world, Frame(0), &dash_right);

    assert_eq!(world.players()[0].motion_state, MotionState::Dash);
    assert_eq!(world.players()[0].position.x, start_x);
    assert_eq!(
        world.players()[0].velocity.x,
        source_units_to_milli(world.players()[0].profile.dash_initial_velocity)
    );
    assert_eq!(
        world.players()[0].dash_entry_velocity_delta,
        0,
        "ftCo_Dash_Phys consumes mv.co.dash.x0 during the same engine frame"
    );

    step_world(&mut world, Frame(1), &dash_right);

    assert!(
        world.players()[0].position.x > start_x,
        "the next frame moves using committed gr_vel"
    );
    assert!(
        world.players()[0].velocity.x
            > source_units_to_milli(world.players()[0].profile.dash_initial_velocity),
        "ordinary Dash_Phys live-stick acceleration runs after xE8 entry staging"
    );
}
```

- [ ] **Step 2: Add a phase-order regression test for Dash IASA before Dash Phys**

Add this test near the Dash action-window tests. It ensures a same-frame action chosen by Dash IASA receives `x54` fall-through behavior exactly from `ftCo_Dash_IASA`, not from later generic Dash physics.

```rust
#[test]
fn dash_iasa_action_runs_before_dash_phys_acceleration() {
    let mut world = World::for_two_players();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(127, 0),
        PlayerInput::neutral(),
    ];
    let side_special_left = [
        PlayerInput::neutral()
            .with_left_stick(-127, 0)
            .with_special(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &dash_right);
    let dash_entry_velocity = world.players()[0].velocity.x;

    step_world(&mut world, Frame(1), &side_special_left);

    assert_eq!(world.players()[0].motion_state, MotionState::SpecialS);
    assert_eq!(
        world.players()[0].velocity.x,
        dash_entry_velocity * world.common_data().dash_velocity_decay_milli / 1000
    );
}
```

- [ ] **Step 3: Run the focused tests and verify red or protective behavior**

Run:

```powershell
cargo test -p mole_core dash_entry_uses_source_xe8_staging_before_next_dash_phys dash_iasa_action_runs_before_dash_phys_acceleration -- --nocapture
```

Expected: the first test may pass if current staging already matches the source-shaped assertion; the second test may pass if existing `dash_iasa_action_state` already protects this path. If both pass, keep them as characterization tests and continue to Task 2 because the architectural replacement is still required by design.

## Task 2: Add Ground Phase Staging Fields Without Changing Behavior

**Files:**
- Modify: `D:\Mole Game\First_Game\crates\mole_core\src\state.rs`
- Modify: `D:\Mole Game\First_Game\crates\mole_core\src\sim.rs`
- Test: `D:\Mole Game\First_Game\crates\mole_core\tests\core_contract.rs`

- [ ] **Step 1: Add deterministic ground staging fields to `PlayerState`**

In `PlayerState`, add fields that mirror the decomp naming without replacing public semantics yet:

```rust
pub ground_velocity_x: i32,
pub ground_accel_x: i32,
pub ground_accel_x2: i32,
```

Initialize them in `PlayerState::new_with_profile`:

```rust
ground_velocity_x: 0,
ground_accel_x: 0,
ground_accel_x2: 0,
```

Add all three fields to checksum mixing next to `velocity` and Dash fields:

```rust
mix_i32(&mut hash, player.ground_velocity_x);
mix_i32(&mut hash, player.ground_accel_x);
mix_i32(&mut hash, player.ground_accel_x2);
```

- [ ] **Step 2: Add a sync helper in `sim.rs`**

Near the ground movement helpers, add:

```rust
fn sync_legacy_velocity_from_ground(player: &mut PlayerState) {
    if player.grounded {
        player.velocity.x = player.ground_velocity_x;
    }
}

fn sync_ground_from_legacy_velocity(player: &mut PlayerState) {
    if player.grounded {
        player.ground_velocity_x = player.velocity.x;
    }
}
```

Call `sync_ground_from_legacy_velocity(player)` at the start of each grounded player's tick before the motion-state match. Call `sync_legacy_velocity_from_ground(player)` after ground acceleration commits.

- [ ] **Step 3: Run existing Dash and checksum tests**

Run:

```powershell
cargo test -p mole_core dash_ checksum -- --nocapture
```

Expected: all selected tests pass. If checksum tests fail because expected values include new deterministic state, update only tests that intentionally pin checksum behavior.

## Task 3: Replace Direct Ground Velocity Mutation With Decomp-Style Staging

**Files:**
- Modify: `D:\Mole Game\First_Game\crates\mole_core\src\sim.rs`
- Test: `D:\Mole Game\First_Game\crates\mole_core\tests\core_contract.rs`

- [ ] **Step 1: Add staging helpers**

Add helpers that match `ftCommon_ApplyFrictionGround`, `ftCommon_8007C98C`, `ftCommon_800804A0`, and fighter velocity commit:

```rust
fn set_ground_friction_accel(player: &mut PlayerState, friction: i32) {
    let friction = friction.abs();
    player.ground_accel_x = if friction > player.ground_velocity_x.abs() {
        -player.ground_velocity_x
    } else if player.ground_velocity_x > 0 {
        -friction
    } else {
        friction
    };
}

fn set_ground_accel_toward_target(
    player: &mut PlayerState,
    mut accel: i32,
    target_velocity: i32,
    friction_per_tick: i32,
) {
    let friction_per_tick = friction_per_tick.abs();
    if target_velocity == 0 {
        set_ground_friction_accel(player, friction_per_tick);
        return;
    }
    if player.ground_velocity_x * accel >= 0 {
        if accel > 0 && player.ground_velocity_x + accel > target_velocity {
            accel = -friction_per_tick;
            if player.ground_velocity_x + accel < target_velocity {
                accel = target_velocity - player.ground_velocity_x;
            }
        } else if accel < 0 && player.ground_velocity_x + accel < target_velocity {
            accel = friction_per_tick;
            if player.ground_velocity_x + accel > target_velocity {
                accel = target_velocity - player.ground_velocity_x;
            }
        }
    }
    player.ground_accel_x = accel;
}

fn set_dash_entry_ground_accel2(player: &mut PlayerState, accel: i32) {
    player.ground_accel_x2 = accel;
}

fn ground_position_delta_x(player: &PlayerState) -> i32 {
    if player.grounded {
        player.ground_velocity_x + player.ground_accel_x
    } else {
        player.velocity.x
    }
}

fn commit_ground_velocity(player: &mut PlayerState) {
    if !player.grounded {
        return;
    }
    player.ground_velocity_x = (player.ground_velocity_x
        + player.ground_accel_x
        + player.ground_accel_x2)
        .clamp(
            -player.profile.ground_max_horizontal_velocity_per_tick,
            player.profile.ground_max_horizontal_velocity_per_tick,
        );
    player.ground_accel_x = 0;
    player.ground_accel_x2 = 0;
    player.velocity.x = player.ground_velocity_x;
}
```

- [ ] **Step 2: Route Dash, Run, RunBrake, TurnRun, Wait, and Landing ground friction through staging**

Replace calls that currently assign `player.velocity.x = apply_ground_accel_toward_target(...)` or `apply_friction_to_zero(...)` for grounded locomotion with staging calls:

```rust
set_ground_accel_toward_target(player, accel, target_velocity, run_ground_friction(player, common_data));
```

For neutral ground friction paths:

```rust
set_ground_friction_accel(player, run_ground_friction(player, common_data));
```

For high-speed Wait friction:

```rust
set_ground_friction_accel(player, traction);
```

- [ ] **Step 3: Update Dash entry to use `ground_accel_x2`**

In `enter_dash`, calculate the source delta from `player.ground_velocity_x` and store it in both the compatibility field and the decomp-shaped staging field:

```rust
let delta = source_dash_entry_velocity_delta(player.ground_velocity_x, direction, player.profile);
player.dash_entry_velocity_delta = delta;
set_dash_entry_ground_accel2(player, delta);
```

- [ ] **Step 4: Commit staged ground velocity in the correct engine phase**

Move the Dash entry delta handling out of the special post-position branch:

```rust
if player.motion_state == MotionState::Dash && player.dash_entry_velocity_delta != 0 {
    player.dash_entry_velocity_delta = 0;
}
```

After the motion-state `phys` logic, compute the flat-ground movement delta with `ground_position_delta_x(player)`, then call `commit_ground_velocity(player)`. Position update for grounded players should use the saved movement delta, not the post-commit `velocity.x`, so `xE8` Dash entry staging does not translate the character on the transition frame.

- [ ] **Step 5: Run focused movement tests**

Run:

```powershell
cargo test -p mole_core dash_ run_state_ turn_run run_brake walk_rollout moonwalk -- --nocapture
```

Expected: all selected tests pass, or failures identify tests whose old assertions encoded the shortcut rather than decomp-shaped behavior.

## Task 4: Split Grounded Locomotion Into Callback-Shaped Helpers

**Files:**
- Modify: `D:\Mole Game\First_Game\crates\mole_core\src\sim.rs`
- Test: `D:\Mole Game\First_Game\crates\mole_core\tests\core_contract.rs`

- [ ] **Step 1: Add helper names that mirror decomp callbacks**

Create focused helpers without changing external behavior:

```rust
fn dash_anim(player: &mut PlayerState) -> bool { /* returns true when Dash animation ended */ }
fn dash_iasa(player: &mut PlayerState, facts: MeleeInputFacts, stick_x: i32, common_data: MeleeCommonData) -> bool { /* returns true if state changed */ }
fn dash_phys(player: &mut PlayerState, stick_x: i32, common_data: MeleeCommonData) { /* stage ground accel */ }
fn dash_coll(player: &mut PlayerState, stage: StageProfile) { /* no-op until floor parity split */ }
```

The body must be moved from the existing Dash match arm. Do not introduce new decisions.

- [ ] **Step 2: Replace the Dash match arm with callback order**

The Dash arm should read structurally like:

```rust
MotionState::Dash => {
    if !player.grounded {
        player.motion_state = MotionState::Air;
        player.motion_frame = 0;
    } else if !dash_iasa(player, input_facts, stick_x, common_data) {
        dash_phys(player, stick_x, common_data);
        if dash_anim(player) {
            exit_dash(player, stick_x, common_data);
        }
    }
}
```

Adjust exact order only when source-backed by `fighter.c` and `ftCo_Dash.c`.

- [ ] **Step 3: Repeat callback-shaped extraction for Run, RunBrake, TurnRun, and Turn only where needed**

For each state, move existing code into `*_iasa`, `*_phys`, and `*_anim` helpers. Keep helper private to `sim.rs`. Do not create a generic callback table yet; this pass is a readable bridge toward one.

- [ ] **Step 4: Run grounded locomotion tests**

Run:

```powershell
cargo test -p mole_core dash_ run_state_ run_brake turn_run standing_turn smash_turn walk_state wait_enters -- --nocapture
```

Expected: all selected tests pass.

## Task 5: Keep Dev Tools Congruent With the New Core Shape

**Files:**
- Modify: `D:\Mole Game\First_Game\docs\state_graphs\mole_current_graph.json`
- Modify if graph node IDs change: `D:\Mole Game\First_Game\config\state_graph_layout.json`
- Modify: `D:\Mole Game\First_Game\docs\research\melee-input-state-reference.md`
- Modify: `D:\Mole Game\First_Game\docs\research\mole-state-coverage-comparison.md`
- Test: `D:\Mole Game\First_Game\tests\test_state_graph_viewer.py`
- Test: `D:\Mole Game\First_Game\tests\test_value_sheets.py`
- Test: `D:\Mole Game\First_Game\tests\test_parity_diff_report.py`

- [ ] **Step 1: Update the Dash node and Dash edges**

In `mole_current_graph.json`, update Dash notes to say the core uses decomp-shaped callback helpers and ground staging fields for Dash/Run locomotion. Keep the known human-feel gap if moonwalk/dashdance is still not confirmed by playtest.

Use wording like:

```json
"notes": "Rust now routes Dash through decomp-shaped anim/input/phys/coll helper phases and stages ground velocity through gr_vel/xE4/xE8 equivalents. Constants remain extracted from PlCo/Falcon data; moonwalk and dashdance remain emergent outcomes rather than named states."
```

- [ ] **Step 2: Update research docs**

In `melee-input-state-reference.md`, replace any wording that says Rust has a simplified Dash state with wording that names:

- `dash_anim`
- `dash_iasa`
- `dash_phys`
- ground staging fields equivalent to `gr_vel`, `xE4_ground_accel_1`, and `xE8_ground_accel_2`

In `mole-state-coverage-comparison.md`, mark Dash/Run core architecture closer to parity, but leave any untested exact timing or playtest feel as partial.

- [ ] **Step 3: Run dev-tool checks**

Run:

```powershell
.venv\Scripts\python.exe tools\generate_value_sheets.py
.venv\Scripts\python.exe tools\export_parity_diff_report.py
.venv\Scripts\python.exe tools\state_graph_viewer.py --check
.venv\Scripts\python.exe -m pytest tests\test_state_graph_viewer.py tests\test_value_sheets.py tests\test_parity_diff_report.py -q
```

Expected: value sheets and parity reports remain stable unless source-backed fields changed. State graph viewer check and tests pass.

## Task 6: Full Verification

**Files:**
- No planned edits.

- [ ] **Step 1: Run Rust formatting and focused core tests**

Run:

```powershell
cargo fmt --check
cargo test -p mole_core -- --nocapture
```

Expected: format check passes and `mole_core` tests pass.

- [ ] **Step 2: Run the whole Rust workspace**

Run:

```powershell
cargo test --workspace --all-features --jobs 1
cargo clippy --workspace --all-targets --all-features --jobs 1 -- -D warnings
```

Expected: all tests pass and clippy reports no warnings.

- [ ] **Step 3: Run Python tool tests**

Run:

```powershell
.venv\Scripts\python.exe -m pytest tests\test_state_graph_viewer.py tests\test_value_sheets.py tests\test_parity_diff_report.py tests\test_launch_inputs.py -q
```

Expected: all selected tests pass.

- [ ] **Step 4: Report test/playtest readiness**

Summarize:

- Whether the branch builds.
- Whether dev tools stayed congruent.
- Which exact movement behaviors changed architecturally.
- Whether the user should test dash dancing/moonwalk feel.

Do not claim Melee parity by feel until human playtest confirms it.
