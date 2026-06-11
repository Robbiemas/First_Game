# Battlefield Floor Friction Parity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Carry the decompilation's floor friction multiplier as a float through the Rust Battlefield stage model and use it in grounded friction exactly where the decomp reads it from floor collision data.

**Architecture:** Add a float friction multiplier to `StageSurface`, treat it as part of the stage asset data, and thread the active floor's multiplier into the grounded traction helpers in `sim.rs`. Update Battlefield's baked stage definition and every `StageSurface` literal in tests so the stage model remains self-consistent and deterministic without any runtime decomp dependency.

**Tech Stack:** Rust, existing `mole_core` stage/collision/simulation code, existing contract tests.

---

### Task 1: Extend stage surfaces with a float friction multiplier

**Files:**
- Modify: `First_Game/crates/mole_core/src/stage.rs`
- Modify: `First_Game/crates/mole_core/src/state.rs`
- Modify: `First_Game/crates/mole_core/src/lib.rs` if re-exports need to stay consistent

- [ ] **Step 1: Update the data model**

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StageSurface {
    pub name: &'static str,
    pub kind: StageSurfaceKind,
    pub left_x: i32,
    pub right_x: i32,
    pub y: i32,
    pub friction_multiplier: f32,
}
```

- [ ] **Step 2: Update stage hashing so the multiplier is deterministic**

```rust
fn mix_stage_surface(hash: &mut u64, surface: StageSurface) {
    mix_str(hash, surface.name);
    mix_u8(hash, stage_surface_kind_id(surface.kind));
    mix_i32(hash, surface.left_x);
    mix_i32(hash, surface.right_x);
    mix_i32(hash, surface.y);
    mix_f32(hash, surface.friction_multiplier);
}
```

- [ ] **Step 3: Keep the stage API compiling by removing `Eq` from the structs that now contain floats**

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StageProfile {
    pub name: &'static str,
    pub main_floor: StageSurface,
    pub soft_platforms: [StageSurface; 3],
    pub blast_zones: StageBlastZones,
    pub spawn_points: [StageSpawnPoint; 4],
}
```

### Task 2: Thread the active floor's multiplier into grounded friction

**Files:**
- Modify: `First_Game/crates/mole_core/src/collision.rs`
- Modify: `First_Game/crates/mole_core/src/sim.rs`
- Modify: `First_Game/crates/mole_core/tests/core_contract.rs`

- [ ] **Step 1: Add a helper that returns the current floor's friction multiplier, defaulting to 1.0 when no floor is present**

```rust
pub fn floor_friction_multiplier_for_bottom(stage: StageProfile, bottom: Vec2) -> f32 {
    floor_surface_for_bottom(stage, bottom)
        .map(|surface| surface.friction_multiplier)
        .unwrap_or(1.0)
}
```

- [ ] **Step 2: Multiply grounded traction and run friction by the active floor multiplier**

```rust
fn apply_ground_traction(player: &mut PlayerState, stage: StageProfile, common_data: MeleeCommonData) {
    let mut traction = player.profile.ground_friction * floor_friction_multiplier_for_bottom(stage, player.position);
    if player.ground_velocity_x.abs() > player.profile.walk_max_velocity {
        traction = traction * common_data.high_speed_ground_friction_multiplier;
    }
    let next_velocity = apply_friction_to_zero(player.ground_velocity_x, traction);
    stage_ground_velocity_x(player, next_velocity);
}

fn run_ground_friction(player: &PlayerState, stage: StageProfile, common_data: MeleeCommonData) -> f32 {
    player.profile.ground_friction
        * common_data.run_ground_friction_multiplier
        * floor_friction_multiplier_for_bottom(stage, player.position)
}
```

- [ ] **Step 3: Update the call sites so the stage is available where friction is applied**

```rust
apply_ground_traction(player, stage, common_data);
apply_run_ground_traction(player, stage, common_data);
```

- [ ] **Step 4: Update or add contract tests that prove Battlefield still uses the same baked stage geometry and that the floor multiplier flows through as a float**

```rust
#[test]
fn battlefield_stage_surface_friction_multiplier_is_a_source_float() {
    let stage = StageProfile::battlefield_test();
    assert_eq!(stage.main_floor.friction_multiplier.to_bits(), 1.0_f32.to_bits());
    assert_eq!(stage.soft_platforms[0].friction_multiplier.to_bits(), 1.0_f32.to_bits());
}
```

### Task 3: Bake Battlefield values into the test stage literals

**Files:**
- Modify: `First_Game/crates/mole_core/src/stage.rs`
- Modify: `First_Game/crates/mole_core/tests/core_contract.rs`

- [ ] **Step 1: Set every Battlefield surface's multiplier explicitly**

```rust
main_floor: StageSurface {
    name: "main_floor",
    kind: StageSurfaceKind::Solid,
    left_x: melee_units_f32(-68.4),
    right_x: melee_units_f32(68.4),
    y: 0,
    friction_multiplier: 1.0,
},
```

- [ ] **Step 2: Update every hand-authored `StageSurface` in tests so the compiler sees the new field on all stage literals**

```rust
StageSurface {
    name: "narrow_main_floor",
    kind: StageSurfaceKind::Solid,
    left_x: -40_000,
    right_x: 40_000,
    y: 0,
    friction_multiplier: 1.0,
}
```

- [ ] **Step 3: Keep the tests focused on parity, not on the extraction pipeline**

```rust
#[test]
fn stage_floor_support_reports_surface_kind() {
    let stage = StageProfile::battlefield_test();
    let floor = stage.main_floor;
    assert_eq!(floor.kind, StageSurfaceKind::Solid);
    assert_eq!(floor.friction_multiplier.to_bits(), 1.0_f32.to_bits());
}
```

---

**Self-review checklist:**
- No runtime dependency on the decompilation remains.
- Battlefield surface data is baked into the Rust stage asset.
- Grounded friction reads the active floor's float multiplier instead of truncating or inventing an integer path.
- All `StageSurface` literals compile with the new field.
