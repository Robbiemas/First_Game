# FallSpecial Platform Collision Parity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Subagents are explicitly disabled for this repository session by the user. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Rust-owned `FallSpecial` soft-platform collision follow the Melee decomp gate from `ftCo_80096CC8`: solid floors always collide, soft platforms collide only when `lstick.y > p_ftCommonData->x25C`.

**Architecture:** Keep the Rust core authoritative. Add the `x25C` common-data field to `MeleeCommonData`, then route only `MotionState::FallSpecial` landing contact through the existing stage-collision `drop_through_soft_platforms` parameter. Do not add a wavedash state or any non-Melee movement shortcut.

**Tech Stack:** Rust `mole_core`, deterministic 60 Hz simulation, existing Battlefield-sized stage profile, Melee decomp reference in `D:\Mole Game\.research\doldecomp-melee`.

---

## Decomp References

- `D:\Mole Game\.research\doldecomp-melee\src\melee\ft\chara\ftCommon\ftCo_FallSpecial.c:133-155`
  - `ftCo_FallSpecial_Coll` calls `ft_80083090(gobj, ftCo_80096CC8, ftCo_80096D28)`.
  - `ftCo_80096CC8` accepts a line when it is not a platform, or when it is a platform and `fp->input.lstick.y > p_ftCommonData->x25C`.
  - `ftCo_80096D28` routes accepted landings into `ftCo_LandingFallSpecial_Enter` when the FallSpecial landing predicate is satisfied.
- `D:\Mole Game\.research\doldecomp-melee\src\melee\ft\types.h:213`
  - `p_ftCommonData->x25C` is a `float`.
- `D:\Mole Game\.research\doldecomp-melee\src\melee\ft\chara\ftCommon\ftCo_EscapeAir.c:110-117`
  - `EscapeAir` uses its own collision callback and lands into `LandingFallSpecial`; this slice does not apply the FallSpecial platform gate to `EscapeAir`.

## File Map

- Modify `D:\Mole Game\First_Game\crates\mole_core\src\common_data.rs`
  - Add `MeleeCommonData::fallspecial_platform_landing_y`.
  - Extract it from PlCo offset `0x25c` via the existing `read_stick_i8` float-to-stick path.
  - Record the source under `input_common_data_field_sources()`.
- Modify `D:\Mole Game\First_Game\crates\mole_core\src\sim.rs`
  - Add a constant for `fallspecial_platform_landing_y`.
  - Add a source-shaped helper whose logic is exactly `stick_y <= x25C` for skipping soft platforms during `MotionState::FallSpecial` contact checks.
  - Pass the resulting boolean into `landing_contact_for_bottom` only for `FallSpecial`.
- Modify `D:\Mole Game\First_Game\crates\mole_core\tests\core_contract.rs`
  - Extend common-data tests for the new field/source.
  - Add Rust simulation tests for neutral/up `FallSpecial` landing on a soft platform and held-down `FallSpecial` skipping that soft platform to the main floor.
  - Add checksum assertions to the held-down path so the behavior is proven through deterministic rollback-owned input snapshots.

---

### Task 1: Expose Melee `x25C` In Common Data

**Files:**
- Modify: `D:\Mole Game\First_Game\crates\mole_core\tests\core_contract.rs`
- Modify: `D:\Mole Game\First_Game\crates\mole_core\src\common_data.rs`

- [ ] **Step 1: Write failing tests for the `x25C` common-data field**

Add these assertions to `input_threshold_defaults_come_from_provisional_common_data`:

```rust
assert_eq!(common.fallspecial_platform_landing_y, -80);
```

Add this block to `input_common_data_sources_track_melee_field_offsets` near the EscapeAir/FallSpecial source checks:

```rust
let fallspecial_platform_landing_y = sources
    .iter()
    .find(|source| source.rust_name == "fallspecial_platform_landing_y")
    .expect("fallspecial_platform_landing_y common-data source should be recorded");
assert_eq!(fallspecial_platform_landing_y.source_name, "x25C");
assert_eq!(fallspecial_platform_landing_y.offset, 0x25c);
```

Add this synthetic PlCo write to `extracted_plco_common_data_reads_big_endian_values_from_source_offsets`:

```rust
put_f32_be(&mut bytes, 0x25c, -0.62);
```

Add this assertion after the existing aerial/common-data assertions:

```rust
assert_eq!(common.fallspecial_platform_landing_y, -79);
```

- [ ] **Step 2: Run the focused common-data test and verify red**

Run:

```powershell
cargo test -p mole_core input_threshold_defaults_come_from_provisional_common_data
```

Expected: compile failure or test failure because `MeleeCommonData` does not yet expose `fallspecial_platform_landing_y`.

- [ ] **Step 3: Add the common-data field and source extraction**

In `MeleeCommonData`, add:

```rust
pub fallspecial_platform_landing_y: i8,
```

In `MeleeCommonData::PROVISIONAL`, add:

```rust
fallspecial_platform_landing_y: -80,
```

In `MeleeCommonData::from_plco_bytes`, after the existing aerial/defensive threshold reads and before `x314`, add:

```rust
data.fallspecial_platform_landing_y =
    read_stick_i8(bytes, 0x25c, "x25C")?;
```

In `INPUT_COMMON_DATA_FIELD_SOURCES`, replace the current `landing_fallspecial_collision` entry with:

```rust
CommonDataFieldSource {
    rust_name: "fallspecial_platform_landing_y",
    source_name: "x25C",
    offset: 0x25c,
    provenance: CommonDataProvenance::ProvisionalMole,
},
```

- [ ] **Step 4: Run the focused common-data tests and verify green**

Run:

```powershell
cargo test -p mole_core input_threshold_defaults_come_from_provisional_common_data
cargo test -p mole_core input_common_data_sources_track_melee_field_offsets
cargo test -p mole_core extracted_plco_common_data_reads_big_endian_values_from_source_offsets
```

Expected: all three tests pass.

- [ ] **Step 5: Commit common-data extraction**

Run:

```powershell
git add crates/mole_core/src/common_data.rs crates/mole_core/tests/core_contract.rs
git commit -m "feat: extract fallspecial platform gate data"
```

---

### Task 2: Apply The FallSpecial Soft-Platform Gate

**Files:**
- Modify: `D:\Mole Game\First_Game\crates\mole_core\tests\core_contract.rs`
- Modify: `D:\Mole Game\First_Game\crates\mole_core\src\sim.rs`

- [ ] **Step 1: Write failing FallSpecial platform-collision tests**

Add these tests near the existing air-dodge landing tests:

```rust
#[test]
fn fall_special_with_stick_above_x25c_lands_on_soft_platform() {
    let mut world = World::for_two_players();
    let stage = world.stage();
    let platform = stage.soft_platforms[0];
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let up_air_dodge = [
        PlayerInput::neutral()
            .with_right_trigger_digital(true)
            .with_left_stick(0, 127),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];

    for frame in 0..4 {
        step_world(&mut world, Frame(frame), &jump);
    }
    step_world(&mut world, Frame(4), &up_air_dodge);

    let mut frame = 5;
    while world.players()[0].motion_state != MotionState::FallSpecial && frame < 80 {
        step_world(&mut world, Frame(frame), &neutral);
        frame += 1;
    }

    assert_eq!(world.players()[0].motion_state, MotionState::FallSpecial);

    while !world.players()[0].grounded && frame < 180 {
        step_world(&mut world, Frame(frame), &neutral);
        frame += 1;
    }

    assert!(world.players()[0].grounded);
    assert_eq!(
        world.players()[0].motion_state,
        MotionState::LandingFallSpecial
    );
    assert_eq!(world.players()[0].position.y, platform.y);
}
```

```rust
#[test]
fn fall_special_holding_down_skips_soft_platform_until_main_floor() {
    let mut world = World::for_two_players();
    let stage = world.stage();
    let platform = stage.soft_platforms[0];
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let up_air_dodge = [
        PlayerInput::neutral()
            .with_right_trigger_digital(true)
            .with_left_stick(0, 127),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let down = [
        PlayerInput::neutral().with_left_stick(
            0,
            MeleeCommonData::provisional_mole().fallspecial_platform_landing_y,
        ),
        PlayerInput::neutral(),
    ];

    for frame in 0..4 {
        step_world(&mut world, Frame(frame), &jump);
    }
    step_world(&mut world, Frame(4), &up_air_dodge);

    let mut frame = 5;
    while world.players()[0].motion_state != MotionState::FallSpecial && frame < 80 {
        step_world(&mut world, Frame(frame), &neutral);
        frame += 1;
    }

    assert_eq!(world.players()[0].motion_state, MotionState::FallSpecial);

    while !world.players()[0].grounded && frame < 220 {
        step_world(&mut world, Frame(frame), &down);
        frame += 1;
        assert_ne!(world.players()[0].position.y, platform.y);
    }

    assert!(world.players()[0].grounded);
    assert_eq!(
        world.players()[0].motion_state,
        MotionState::LandingFallSpecial
    );
    assert_eq!(world.players()[0].position.y, stage.main_floor.y);
}
```

- [ ] **Step 2: Run the FallSpecial platform test and verify red**

Run:

```powershell
cargo test -p mole_core fall_special_holding_down_skips_soft_platform_until_main_floor
```

Expected: fail because the current simulation calls `landing_contact_for_bottom(..., false)` for `FallSpecial`, so held down still lands on the soft platform.

- [ ] **Step 3: Implement the decomp-shaped FallSpecial gate in simulation**

In `D:\Mole Game\First_Game\crates\mole_core\src\sim.rs`, add this constant near the other common-data constants:

```rust
const FALLSPECIAL_PLATFORM_LANDING_Y: i8 =
    crate::common_data::MeleeCommonData::PROVISIONAL.fallspecial_platform_landing_y;
```

Add this helper near the other small simulation helpers:

```rust
fn fall_special_skips_soft_platforms(stick_y: i8) -> bool {
    stick_y <= FALLSPECIAL_PLATFORM_LANDING_Y
}
```

Replace the generic non-`EscapeAir` landing contact call:

```rust
if let Some(contact) =
    landing_contact_for_bottom(stage, previous_position, player.position, false)
{
```

with:

```rust
let drop_through_soft_platforms = player.motion_state == MotionState::FallSpecial
    && fall_special_skips_soft_platforms(stick_y);
if let Some(contact) = landing_contact_for_bottom(
    stage,
    previous_position,
    player.position,
    drop_through_soft_platforms,
) {
```

Keep the `EscapeAir` branch passing `false`, because this plan is mirroring `ftCo_FallSpecial_Coll`, not `ftCo_EscapeAir_Coll`.

- [ ] **Step 4: Run the focused FallSpecial tests and verify green**

Run:

```powershell
cargo test -p mole_core fall_special_with_stick_above_x25c_lands_on_soft_platform
cargo test -p mole_core fall_special_holding_down_skips_soft_platform_until_main_floor
```

Expected: both tests pass.

- [ ] **Step 5: Commit FallSpecial collision gate**

Run:

```powershell
git add crates/mole_core/src/sim.rs crates/mole_core/tests/core_contract.rs
git commit -m "feat: gate fallspecial platform collision"
```

---

### Task 3: Lock Rollback Determinism For The New Gate

**Files:**
- Modify: `D:\Mole Game\First_Game\crates\mole_core\tests\core_contract.rs`

- [ ] **Step 1: Add checksum coverage for held-down FallSpecial platform skip**

Add this test near `fall_special_holding_down_skips_soft_platform_until_main_floor`:

```rust
#[test]
fn fall_special_platform_gate_is_deterministic_from_input_snapshots() {
    let mut left = World::for_two_players();
    let mut right = World::for_two_players();
    let stage = left.stage();
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let up_air_dodge = [
        PlayerInput::neutral()
            .with_right_trigger_digital(true)
            .with_left_stick(0, 127),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let down = [
        PlayerInput::neutral().with_left_stick(
            0,
            MeleeCommonData::provisional_mole().fallspecial_platform_landing_y,
        ),
        PlayerInput::neutral(),
    ];

    for frame in 0..4 {
        step_world(&mut left, Frame(frame), &jump);
        step_world(&mut right, Frame(frame), &jump);
        assert_eq!(left.checksum(), right.checksum());
    }
    step_world(&mut left, Frame(4), &up_air_dodge);
    step_world(&mut right, Frame(4), &up_air_dodge);
    assert_eq!(left.checksum(), right.checksum());

    let mut frame = 5;
    while left.players()[0].motion_state != MotionState::FallSpecial && frame < 80 {
        step_world(&mut left, Frame(frame), &neutral);
        step_world(&mut right, Frame(frame), &neutral);
        assert_eq!(left.checksum(), right.checksum());
        frame += 1;
    }

    while !left.players()[0].grounded && frame < 220 {
        step_world(&mut left, Frame(frame), &down);
        step_world(&mut right, Frame(frame), &down);
        assert_eq!(left.checksum(), right.checksum());
        frame += 1;
    }

    assert!(left.players()[0].grounded);
    assert_eq!(left.players()[0].position.y, stage.main_floor.y);
    assert_eq!(left.snapshot(), right.snapshot());
}
```

- [ ] **Step 2: Run the checksum test**

Run:

```powershell
cargo test -p mole_core fall_special_platform_gate_is_deterministic_from_input_snapshots
```

Expected: pass after Task 2, with deterministic checksums matching frame-by-frame.

- [ ] **Step 3: Commit deterministic coverage**

Run:

```powershell
git add crates/mole_core/tests/core_contract.rs
git commit -m "test: cover fallspecial platform gate determinism"
```

---

### Task 4: Verify The Slice

**Files:**
- No source changes expected.

- [ ] **Step 1: Run focused Rust core tests**

Run:

```powershell
cargo test -p mole_core fall_special
```

Expected: all `fall_special`-named tests pass.

- [ ] **Step 2: Run the full core crate**

Run:

```powershell
cargo test -p mole_core
```

Expected: all `mole_core` tests pass.

- [ ] **Step 3: Run workspace formatting and lint checks**

Run:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
git diff --check
rg -n "unsafe\s*\{" crates
```

Expected:
- `cargo fmt` passes.
- `cargo clippy` passes.
- `git diff --check` passes.
- `rg -n "unsafe\s*\{" crates` prints no matches and exits with code `1`.

- [ ] **Step 4: Review commit history and status**

Run:

```powershell
git status --short --branch
git log --oneline -5
```

Expected:
- Branch remains `handoff/rust-rollback-architecture`.
- Only the pre-existing untracked `config/state_graph_layout.json` may remain uncommitted.
- The new commits are visible at the top of the branch.

