# Air Dodge Landing Movement Parity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the Rust core's air-dodge landing movement feel more Melee-shaped by routing EscapeAir/FallSpecial landing through deterministic stage contact, without adding any wavedash-specific state or mechanic.

**Architecture:** Keep gameplay authority in `mole_core`. Introduce a small stage-contact helper that resolves vertical landing against `StageProfile` surfaces, then make `step_world` use it for ordinary landing and air-dodge landing state selection. Preserve existing `KneeBend`, `EscapeAir`, `FallSpecial`, and `LandingFallSpecial` states; a wavedash-like slide must emerge only from those states plus collision and traction.

**Tech Stack:** Rust workspace, `mole_core` deterministic simulation, existing `StageProfile`, `EcbDiamond`, `PlayerState`, `World`, and `core_contract` tests.

---

## File Map

- Modify: `crates/mole_core/src/stage.rs`
  - Add deterministic surface iteration over main floor plus soft platforms.
- Modify: `crates/mole_core/src/state.rs`
  - Store `StageProfile` in `World` so simulation can resolve contact against the Battlefield-like stage.
  - Include stage identity/surfaces in checksum if the stage becomes world state.
- Modify: `crates/mole_core/src/sim.rs`
  - Replace direct `GROUND_Y` landing checks with a helper that resolves stage contact and calls `enter_landing` or `enter_landing_fall_special`.
  - Keep EscapeAir/FallSpecial state flow unchanged except for contact source.
- Modify: `crates/mole_core/tests/core_contract.rs`
  - Add TDD coverage for no `Wavedash` state, soft-platform landing, and landing-fall-special slide through the stage contact path.
- Modify: `docs/research/melee-input-state-reference.md`
  - Document the contact helper and remaining collision gaps.
- Modify: `docs/research/melee-data-provenance-audit.md`
  - Mark collision/contact as Rust-owned and list remaining platform/ledge data gaps.

---

### Task 1: Prove There Is No Wavedash State

**Files:**
- Test: `crates/mole_core/tests/core_contract.rs`

- [ ] **Step 1: Write the failing/no-regression test**

Add this test near the existing `escape_air_landing_enters_landing_fall_special_instead_of_wait` tests:

```rust
#[test]
fn air_dodge_landing_uses_existing_melee_states_not_wavedash_state() {
    let mut world = World::for_two_players();
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let down_air_dodge = [
        PlayerInput::neutral()
            .with_right_trigger_digital(true)
            .with_left_stick(80, -127),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];

    for frame in 0..4 {
        step_world(&mut world, Frame(frame), &jump);
    }
    step_world(&mut world, Frame(4), &down_air_dodge);

    assert_eq!(world.players()[0].motion_state, MotionState::EscapeAir);

    let mut frame = 5;
    while !world.players()[0].grounded && frame < 80 {
        step_world(&mut world, Frame(frame), &neutral);
        assert_ne!(
            format!("{:?}", world.players()[0].motion_state),
            "Wavedash"
        );
        frame += 1;
    }

    assert_eq!(
        world.players()[0].motion_state,
        MotionState::LandingFallSpecial
    );
    assert!(world.players()[0].velocity.x > 0);
}
```

- [ ] **Step 2: Run the focused test**

Run:

```powershell
cargo test -p mole_core air_dodge_landing_uses_existing_melee_states_not_wavedash_state
```

Expected: PASS. This is a guardrail test; it should pass before implementation and continue to pass after every task.

- [ ] **Step 3: Commit if this is the only change**

If no other files are modified yet, commit:

```powershell
git add crates\mole_core\tests\core_contract.rs
git commit -m "test: assert air dodge landing uses existing states"
git push origin handoff/rust-rollback-architecture
```

---

### Task 2: Add Stage Surface Iteration

**Files:**
- Modify: `crates/mole_core/src/stage.rs`
- Test: `crates/mole_core/tests/core_contract.rs`

- [ ] **Step 1: Write the failing test**

Add this test near `default_stage_is_battlefield_sized_in_core_units`:

```rust
#[test]
fn stage_profile_lists_main_floor_before_soft_platforms_for_contact() {
    let stage = StageProfile::battlefield_test();
    let surfaces = stage.collision_surfaces();

    assert_eq!(surfaces.len(), 4);
    assert_eq!(surfaces[0].name, "main_floor");
    assert_eq!(surfaces[0].kind, StageSurfaceKind::Solid);
    assert_eq!(surfaces[1].name, "left_platform");
    assert_eq!(surfaces[1].kind, StageSurfaceKind::Soft);
    assert_eq!(surfaces[2].name, "right_platform");
    assert_eq!(surfaces[2].kind, StageSurfaceKind::Soft);
    assert_eq!(surfaces[3].name, "top_platform");
    assert_eq!(surfaces[3].kind, StageSurfaceKind::Soft);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```powershell
cargo test -p mole_core stage_profile_lists_main_floor_before_soft_platforms_for_contact
```

Expected: FAIL with no method named `collision_surfaces`.

- [ ] **Step 3: Implement the minimal method**

In `crates/mole_core/src/stage.rs`, add:

```rust
impl StageProfile {
    pub const fn collision_surfaces(self) -> [StageSurface; 4] {
        [
            self.main_floor,
            self.soft_platforms[0],
            self.soft_platforms[1],
            self.soft_platforms[2],
        ]
    }
}
```

Keep the existing `battlefield_test` method in the same `impl StageProfile` block or add this method to that block.

- [ ] **Step 4: Run test to verify it passes**

Run:

```powershell
cargo test -p mole_core stage_profile_lists_main_floor_before_soft_platforms_for_contact
```

Expected: PASS.

- [ ] **Step 5: Commit**

```powershell
git add crates\mole_core\src\stage.rs crates\mole_core\tests\core_contract.rs
git commit -m "feat: expose deterministic stage collision surfaces"
git push origin handoff/rust-rollback-architecture
```

---

### Task 3: Store Stage Profile In World

**Files:**
- Modify: `crates/mole_core/src/state.rs`
- Test: `crates/mole_core/tests/core_contract.rs`

- [ ] **Step 1: Write the failing test**

Add this test near the stage profile tests:

```rust
#[test]
fn world_owns_battlefield_stage_for_deterministic_contact() {
    let world = World::for_two_players();
    let stage = world.stage();

    assert_eq!(stage.name, "battlefield_test");
    assert_eq!(stage.main_floor.name, "main_floor");
    assert_eq!(stage.soft_platforms.len(), 3);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```powershell
cargo test -p mole_core world_owns_battlefield_stage_for_deterministic_contact
```

Expected: FAIL with no method named `stage`.

- [ ] **Step 3: Add stage to World**

In `crates/mole_core/src/state.rs`, update imports:

```rust
use crate::{
    time::Frame, MeleeInputFacts, MeleeInputSnapshot, MeleeInputTimers, MeleeJumpInput, PlayerInput,
    StageProfile,
};
```

Add a field to `World`:

```rust
stage: StageProfile,
```

Initialize it in `World::for_two_players_with_profiles`:

```rust
stage: StageProfile::battlefield_test(),
```

Add the getter:

```rust
pub const fn stage(&self) -> StageProfile {
    self.stage
}
```

- [ ] **Step 4: Include stage in checksum**

In `World::checksum`, before hashing players, add:

```rust
mix_stage_profile(&mut hash, self.stage);
```

Add helper functions near `mix_fighter_profile`:

```rust
fn mix_stage_profile(hash: &mut u64, stage: StageProfile) {
    for byte in stage.name.as_bytes() {
        mix_u8(hash, *byte);
    }
    mix_stage_surface(hash, stage.main_floor);
    for surface in stage.soft_platforms {
        mix_stage_surface(hash, surface);
    }
    mix_i32(hash, stage.blast_zones.left_x);
    mix_i32(hash, stage.blast_zones.right_x);
    mix_i32(hash, stage.blast_zones.top_y);
    mix_i32(hash, stage.blast_zones.bottom_y);
}

fn mix_stage_surface(hash: &mut u64, surface: crate::StageSurface) {
    for byte in surface.name.as_bytes() {
        mix_u8(hash, *byte);
    }
    mix_u8(
        hash,
        match surface.kind {
            crate::StageSurfaceKind::Solid => 0,
            crate::StageSurfaceKind::Soft => 1,
        },
    );
    mix_i32(hash, surface.left_x);
    mix_i32(hash, surface.right_x);
    mix_i32(hash, surface.y);
}
```

- [ ] **Step 5: Run test to verify it passes**

Run:

```powershell
cargo test -p mole_core world_owns_battlefield_stage_for_deterministic_contact
```

Expected: PASS.

- [ ] **Step 6: Run checksum regression**

Run:

```powershell
cargo test -p mole_core same_start_and_inputs_produce_same_checksum
```

Expected: PASS.

- [ ] **Step 7: Commit**

```powershell
git add crates\mole_core\src\state.rs crates\mole_core\tests\core_contract.rs
git commit -m "feat: store stage profile in rollback world"
git push origin handoff/rust-rollback-architecture
```

---

### Task 4: Add Deterministic Vertical Landing Contact

**Files:**
- Modify: `crates/mole_core/src/collision.rs`
- Modify: `crates/mole_core/src/lib.rs`
- Test: `crates/mole_core/tests/core_contract.rs`

- [ ] **Step 1: Write failing tests**

Add these tests near the ECB tests:

```rust
#[test]
fn vertical_stage_contact_lands_on_main_floor_when_crossing_downward() {
    let stage = StageProfile::battlefield_test();
    let contact = mole_core::landing_contact_for_bottom(
        stage,
        Vec2 { x: 0, y: 2_000 },
        Vec2 { x: 0, y: -500 },
        false,
    )
    .expect("downward crossing should contact main floor");

    assert_eq!(contact.surface.name, "main_floor");
    assert_eq!(contact.surface.kind, StageSurfaceKind::Solid);
    assert_eq!(contact.y, 0);
}

#[test]
fn vertical_stage_contact_lands_on_soft_platform_when_enabled() {
    let stage = StageProfile::battlefield_test();
    let platform = stage.soft_platforms[2];
    let contact = mole_core::landing_contact_for_bottom(
        stage,
        Vec2 {
            x: 0,
            y: platform.y + 2_000,
        },
        Vec2 {
            x: 0,
            y: platform.y - 500,
        },
        false,
    )
    .expect("downward crossing should contact top platform");

    assert_eq!(contact.surface.name, "top_platform");
    assert_eq!(contact.surface.kind, StageSurfaceKind::Soft);
    assert_eq!(contact.y, platform.y);
}

#[test]
fn vertical_stage_contact_ignores_soft_platform_when_dropping_through() {
    let stage = StageProfile::battlefield_test();
    let platform = stage.soft_platforms[2];
    let contact = mole_core::landing_contact_for_bottom(
        stage,
        Vec2 {
            x: 0,
            y: platform.y + 2_000,
        },
        Vec2 {
            x: 0,
            y: platform.y - 500,
        },
        true,
    );

    assert!(contact.is_none());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```powershell
cargo test -p mole_core vertical_stage_contact
```

Expected: FAIL with no exported `landing_contact_for_bottom`.

- [ ] **Step 3: Implement contact types and helper**

In `crates/mole_core/src/collision.rs`, update imports:

```rust
use crate::{stage::StageSurfaceKind, StageProfile, StageSurface};
use crate::state::Vec2;
```

Add:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageLandingContact {
    pub surface: StageSurface,
    pub y: i32,
}

pub fn landing_contact_for_bottom(
    stage: StageProfile,
    previous_bottom: Vec2,
    current_bottom: Vec2,
    drop_through_soft_platforms: bool,
) -> Option<StageLandingContact> {
    if current_bottom.y > previous_bottom.y {
        return None;
    }

    let mut best: Option<StageLandingContact> = None;
    for surface in stage.collision_surfaces() {
        if drop_through_soft_platforms && surface.kind == StageSurfaceKind::Soft {
            continue;
        }
        if current_bottom.x < surface.left_x || current_bottom.x > surface.right_x {
            continue;
        }
        if previous_bottom.y >= surface.y && current_bottom.y <= surface.y {
            let contact = StageLandingContact {
                surface,
                y: surface.y,
            };
            if best
                .map(|existing| contact.y > existing.y)
                .unwrap_or(true)
            {
                best = Some(contact);
            }
        }
    }
    best
}
```

- [ ] **Step 4: Export the helper**

In `crates/mole_core/src/lib.rs`, change:

```rust
pub use collision::EcbDiamond;
```

to:

```rust
pub use collision::{landing_contact_for_bottom, EcbDiamond, StageLandingContact};
```

- [ ] **Step 5: Run tests to verify they pass**

Run:

```powershell
cargo test -p mole_core vertical_stage_contact
```

Expected: PASS.

- [ ] **Step 6: Commit**

```powershell
git add crates\mole_core\src\collision.rs crates\mole_core\src\lib.rs crates\mole_core\tests\core_contract.rs
git commit -m "feat: add deterministic vertical stage contact"
git push origin handoff/rust-rollback-architecture
```

---

### Task 5: Route World Landing Through Stage Contact

**Files:**
- Modify: `crates/mole_core/src/sim.rs`
- Test: `crates/mole_core/tests/core_contract.rs`

- [ ] **Step 1: Write the failing soft-platform landing test**

Add this test near `ordinary_airborne_landing_enters_landing_not_wait`:

```rust
#[test]
fn ordinary_airborne_contact_can_land_on_soft_platform() {
    let mut world = World::for_two_players();
    let stage = world.stage();
    let platform = stage.soft_platforms[0];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];

    for frame in 0..4 {
        step_world(&mut world, Frame(frame), &jump);
    }

    let mut frame = 4;
    while !world.players()[0].grounded && frame < 120 {
        step_world(&mut world, Frame(frame), &neutral);
        frame += 1;
        if world.players()[0].grounded {
            break;
        }
    }

    assert!(world.players()[0].grounded);
    assert_eq!(world.players()[0].motion_state, MotionState::Landing);
    assert_eq!(world.players()[0].position.y, platform.y);
}
```

This test should currently fail because world landing snaps only to `GROUND_Y`.

- [ ] **Step 2: Run test to verify it fails**

Run:

```powershell
cargo test -p mole_core ordinary_airborne_contact_can_land_on_soft_platform
```

Expected: FAIL because player lands at `0` instead of the left platform y.

- [ ] **Step 3: Replace direct ground snap with contact helper**

In `crates/mole_core/src/sim.rs`, import the helper:

```rust
use crate::collision::landing_contact_for_bottom;
```

Inside the per-player loop, before updating `player.position.x`, store:

```rust
let previous_position = player.position;
```

For the non-grounded landing logic, replace each direct `player.position.y <= GROUND_Y` landing branch with this contact-driven shape:

```rust
if let Some(contact) = landing_contact_for_bottom(
    world.stage(),
    previous_position,
    player.position,
    false,
) {
    let landing_state = player.motion_state;
    player.position.y = contact.y;
    player.velocity.y = 0;
    player.grounded = true;
    player.fast_falling = false;
    player.jumps_remaining = player.profile.max_jumps;
    player.jump_input = Default::default();
    player.short_hop = false;
    if matches!(
        landing_state,
        MotionState::EscapeAir | MotionState::FallSpecial
    ) {
        enter_landing_fall_special(player);
    } else {
        enter_landing(player);
    }
}
```

For the EscapeAir branch, use the same contact helper and call `enter_landing_fall_special(player)` on contact.

- [ ] **Step 4: Preserve main-floor behavior**

Run:

```powershell
cargo test -p mole_core ordinary_airborne_landing_enters_landing_not_wait escape_air_landing_enters_landing_fall_special_instead_of_wait landing_fall_special_preserves_slide_before_returning_to_wait
```

Expected: If Cargo rejects multiple filters, run these three commands individually. Each should PASS after the contact helper is wired.

- [ ] **Step 5: Run soft-platform test**

Run:

```powershell
cargo test -p mole_core ordinary_airborne_contact_can_land_on_soft_platform
```

Expected: PASS.

- [ ] **Step 6: Commit**

```powershell
git add crates\mole_core\src\sim.rs crates\mole_core\tests\core_contract.rs
git commit -m "feat: route landings through stage contact"
git push origin handoff/rust-rollback-architecture
```

---

### Task 6: Add A Floor-Support Helper For Landing States

**Files:**
- Modify: `crates/mole_core/src/collision.rs`
- Modify: `crates/mole_core/src/lib.rs`
- Modify: `crates/mole_core/src/sim.rs`
- Test: `crates/mole_core/tests/core_contract.rs`

- [ ] **Step 1: Write the failing test**

Add this test near the other stage collision tests:

```rust
#[test]
fn stage_floor_support_requires_surface_range_and_matching_height() {
    let stage = StageProfile::battlefield_test();
    let floor = stage.main_floor;

    assert!(mole_core::has_floor_support(
        stage,
        Vec2 {
            x: floor.left_x,
            y: floor.y,
        }
    ));
    assert!(mole_core::has_floor_support(
        stage,
        Vec2 {
            x: floor.right_x,
            y: floor.y,
        }
    ));
    assert!(!mole_core::has_floor_support(
        stage,
        Vec2 {
            x: floor.left_x - 1,
            y: floor.y,
        }
    ));
    assert!(!mole_core::has_floor_support(
        stage,
        Vec2 {
            x: 0,
            y: floor.y + 1,
        }
    ));
}
```

- [ ] **Step 2: Run test to verify current behavior**

Run:

```powershell
cargo test -p mole_core stage_floor_support_requires_surface_range_and_matching_height
```

Expected: FAIL because `has_floor_support` is not exported yet.

- [ ] **Step 3: Implement support check**

Add a helper in `collision.rs`:

```rust
pub fn has_floor_support(stage: StageProfile, bottom: Vec2) -> bool {
    stage.collision_surfaces().iter().any(|surface| {
        bottom.y == surface.y && bottom.x >= surface.left_x && bottom.x <= surface.right_x
    })
}
```

Export it from `lib.rs`:

```rust
pub use collision::{
    has_floor_support, landing_contact_for_bottom, EcbDiamond, StageLandingContact,
};
```

Import it in `sim.rs`:

```rust
use crate::collision::{has_floor_support, landing_contact_for_bottom};
```

Copy the stage before the player loop:

```rust
let stage = world.stage();
```

At the top of the `MotionState::LandingFallSpecial` and `MotionState::Landing` branches, before traction or frame advancement:

```rust
if !has_floor_support(stage, player.position) {
    player.grounded = false;
    player.motion_state = if matches!(player.motion_state, MotionState::LandingFallSpecial) {
        MotionState::FallSpecial
    } else {
        MotionState::Air
    };
    player.motion_frame = 0;
    continue;
}
```

- [ ] **Step 4: Verify narrow behavior**

Run:

```powershell
cargo test -p mole_core stage_floor_support_requires_surface_range_and_matching_height
cargo test -p mole_core landing_fall_special
cargo test -p mole_core ordinary_landing
```

Expected: all listed tests pass.

- [ ] **Step 5: Commit**

```powershell
git add crates\mole_core\src\collision.rs crates\mole_core\src\lib.rs crates\mole_core\src\sim.rs crates\mole_core\tests\core_contract.rs
git commit -m "feat: add stage floor support checks"
git push origin handoff/rust-rollback-architecture
```

---

### Task 7: Document The Contact Path And Remaining Gaps

**Files:**
- Modify: `docs/research/melee-input-state-reference.md`
- Modify: `docs/research/melee-data-provenance-audit.md`

- [ ] **Step 1: Update movement reference docs**

In `docs/research/melee-input-state-reference.md`, update the Landing / EscapeAir section with this paragraph:

```markdown
Current Rust core status: airborne landing and air-dodge landing now route
through a deterministic stage-contact helper instead of a hardcoded `y = 0`
floor snap. The helper checks the Battlefield-like main floor and soft platforms
in Rust core units, chooses the highest crossed surface under the ECB bottom,
and preserves the existing Melee state split: ordinary contact enters
`Landing`, while `EscapeAir`/`FallSpecial` contact enters
`LandingFallSpecial`. This is still a vertical-contact slice; ledges, walls,
ceilings, cliff catch, and full platform drop-through timing remain separate
collision tasks.
```

- [ ] **Step 2: Update provenance audit**

In `docs/research/melee-data-provenance-audit.md`, add this bullet under current decomp-shaped logic:

```markdown
- Landing state selection now uses a Rust-owned stage-contact helper over
  `StageProfile` surfaces instead of a simulator-local hardcoded floor snap.
  This keeps air-dodge landing movement deterministic and prepares the collision
  path for platform/drop-through parity.
```

Add this active data gap:

```markdown
- Full Melee collision parity still needs ledges, walls, ceilings, cliff catch,
  pass-through platform timing, and source-accurate collision callbacks.
```

- [ ] **Step 3: Run docs diff check**

Run:

```powershell
git diff --check
```

Expected: exit 0, apart from harmless CRLF warnings on Windows.

- [ ] **Step 4: Commit**

```powershell
git add docs\research\melee-input-state-reference.md docs\research\melee-data-provenance-audit.md
git commit -m "docs: record air dodge landing contact path"
git push origin handoff/rust-rollback-architecture
```

---

### Task 8: Full Verification Gate

**Files:**
- No new source files.

- [ ] **Step 1: Run full Rust tests**

Run:

```powershell
cargo test --workspace
```

Expected: all tests pass.

- [ ] **Step 2: Run SDL/WUP runtime feature tests**

Run:

```powershell
cargo test -p mole_runtime --features "sdl wup"
```

Expected: all tests pass.

- [ ] **Step 3: Run clippy**

Run:

```powershell
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Expected: exit 0.

- [ ] **Step 4: Run formatting and diff checks**

Run:

```powershell
cargo fmt --all -- --check
git diff --check
```

Expected: exit 0. Windows CRLF warnings are acceptable if there are no whitespace errors.

- [ ] **Step 5: Confirm no unsafe blocks were introduced**

Run:

```powershell
rg -n "unsafe\s*\{" crates
```

Expected: exit 1 with no matches.

- [ ] **Step 6: Run state graph check**

Run:

```powershell
.venv\Scripts\python.exe tools\state_graph_viewer.py --check
```

Expected: graph summary prints and command exits 0.

- [ ] **Step 7: Commit any verification-only docs/test updates**

If files changed during verification, commit them. If no files changed, do not create an empty commit.

---

## Self-Review

- Spec coverage: This plan covers the approved first target: Melee mechanics that produce wavedashing, without adding a wavedash state.
- Scope: This plan intentionally does not implement hitboxes, hurtboxes, damage, knockback, hitlag, shield stun, ledges, walls, ceilings, or full shield parity.
- TDD: Each behavior change begins with a focused failing test, except Task 1, which is a guardrail expected to pass before implementation.
- Data ownership: New contact uses `StageProfile`; no gameplay value is moved into Pygame or runtime host code.
