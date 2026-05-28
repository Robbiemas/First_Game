# Melee Unit Visual Parity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the Rust runtime use explicit Melee-style core units for Falcon-modeled Dolphin Mole gameplay, Battlefield-sized stage geometry, sprite scaling, four-point diamond ECB overlays, and frame logs.

**Architecture:** `mole_core` owns deterministic unit/profile/stage/ECB data. `mole_runtime` owns rendering transforms, sprite/image metadata, SDL drawing, and debug logs derived from immutable core snapshots. Pygame assets remain visual references only.

**Tech Stack:** Rust workspace, `mole_core`, `mole_runtime`, SDL3 feature path, native WUP input, existing Rust integration tests.

---

## File Structure

- Create `crates/mole_core/src/units.rs`: deterministic Melee-unit scale constants and integer conversion helpers.
- Create `crates/mole_core/src/stage.rs`: `StageProfile`, `StageSurface`, Battlefield-sized platform data in core units.
- Create `crates/mole_core/src/collision.rs`: four-point `EcbDiamond` helpers from active scaled profile bounds.
- Modify `crates/mole_core/src/state.rs`: expand `FighterProfile` with Falcon-modeled movement values and visual/collision dimensions.
- Modify `crates/mole_core/src/sim.rs`: use `FighterProfile` for gravity, fall speed, fast fall, dash/run speed, traction, and existing walk fields.
- Modify `crates/mole_core/src/lib.rs`: export the new unit, stage, and ECB types.
- Modify `crates/mole_core/tests/core_contract.rs`: add focused contracts for units, Falcon values, Battlefield stage data, ECB, and deterministic snapshots.
- Modify `crates/mole_runtime/src/assets.rs`: add sprite source dimensions and uniform Dolphin Mole scaling metadata.
- Modify `crates/mole_runtime/src/lib.rs`: replace the hidden world-to-screen constant with `RenderTransform`; emit stage surfaces, sprite rects, and ECB points.
- Modify `crates/mole_runtime/src/main.rs`: draw stage surfaces, sprite fallback rects, and ECB lines; add optional frame log output.
- Modify `crates/mole_runtime/tests/runtime_contract.rs`: add render-transform, sprite-scale, ECB, and log tests.

---

### Task 1: Core Unit, Stage, and ECB Contracts

**Files:**
- Create: `crates/mole_core/src/units.rs`
- Create: `crates/mole_core/src/stage.rs`
- Create: `crates/mole_core/src/collision.rs`
- Modify: `crates/mole_core/src/lib.rs`
- Modify: `crates/mole_core/src/state.rs`
- Test: `crates/mole_core/tests/core_contract.rs`

- [ ] **Step 1: Write failing core tests**

Add tests named:

```rust
#[test]
fn melee_units_use_milli_units_for_public_falcon_values() {
    assert_eq!(melee_units(2.3), 2_300);
    assert_eq!(melee_units(0.13), 130);
}

#[test]
fn default_stage_is_battlefield_sized_in_core_units() {
    let stage = StageProfile::battlefield_test();
    assert_eq!(stage.name, "battlefield_test");
    assert_eq!(stage.main_floor.left_x, melee_units_f32(-68.4000015259));
    assert_eq!(stage.main_floor.right_x, melee_units_f32(68.4000015259));
    assert_eq!(stage.soft_platforms.len(), 3);
    assert!(stage.soft_platforms.iter().all(|surface| surface.kind == StageSurfaceKind::Soft));
}

#[test]
fn ecb_diamond_uses_four_midpoint_vertices() {
    let ecb = EcbDiamond::from_bottom_center_and_size(Vec2 { x: 100, y: 0 }, 62_000, 136_000);
    assert_eq!(ecb.top, Vec2 { x: 100, y: 136_000 });
    assert_eq!(ecb.right, Vec2 { x: 31_100, y: 68_000 });
    assert_eq!(ecb.bottom, Vec2 { x: 100, y: 0 });
    assert_eq!(ecb.left, Vec2 { x: -30_900, y: 68_000 });
    assert_eq!(ecb.points(), [ecb.top, ecb.right, ecb.bottom, ecb.left]);
}
```

- [ ] **Step 2: Verify RED**

Run: `cargo test -p mole_core melee_units_use_milli_units_for_public_falcon_values default_stage_is_battlefield_sized_in_core_units ecb_diamond_uses_four_midpoint_vertices`

Expected: fails because the new types and helpers do not exist.

- [ ] **Step 3: Implement minimal core unit/stage/ECB data**

Add:

```rust
pub const MELEE_UNIT_SCALE: i32 = 1_000;

pub const fn melee_units(value: f32) -> i32 {
    (value * MELEE_UNIT_SCALE as f32) as i32
}

pub fn melee_units_f32(value: f32) -> i32 {
    (value * MELEE_UNIT_SCALE as f32).round() as i32
}
```

Add `StageSurfaceKind::{Solid, Soft}`, `StageSurface { name, kind, left_x, right_x, y }`, and `StageProfile::battlefield_test()` using libmelee Battlefield values:

```text
main floor: x -68.4000015259..68.4000015259, y 0
left platform: x -57.60000228881836..-20, y 27.20009994506836
right platform: x 20..57.60000228881836, y 27.20009994506836
top platform: x -18.80000114440918..18.80000114440918, y 54.40010070800781
blast zones: -224, 224, 200, -108.8
```

Add `EcbDiamond` with exactly four `Vec2` vertices: top, right, bottom, left.

- [ ] **Step 4: Verify GREEN**

Run: `cargo test -p mole_core melee_units_use_milli_units_for_public_falcon_values default_stage_is_battlefield_sized_in_core_units ecb_diamond_uses_four_midpoint_vertices`

Expected: pass.

- [ ] **Step 5: Commit**

Run:

```bash
git add crates/mole_core/src/units.rs crates/mole_core/src/stage.rs crates/mole_core/src/collision.rs crates/mole_core/src/lib.rs crates/mole_core/src/state.rs crates/mole_core/tests/core_contract.rs
git commit -m "feat: add melee unit stage and ecb profiles"
```

---

### Task 2: Falcon-Modeled Fighter Profile

**Files:**
- Modify: `crates/mole_core/src/state.rs`
- Modify: `crates/mole_core/src/sim.rs`
- Test: `crates/mole_core/tests/core_contract.rs`

- [ ] **Step 1: Write failing profile tests**

Add tests named:

```rust
#[test]
fn falcon_like_profile_exposes_public_falcon_gameplay_values() {
    let profile = FighterProfile::falcon_like();
    assert_eq!(profile.reference_character, "captain_falcon");
    assert_eq!(profile.run_speed_per_tick, 2_300);
    assert_eq!(profile.initial_dash_speed_per_tick, 2_000);
    assert_eq!(profile.walk_speed_per_tick, 850);
    assert_eq!(profile.traction_per_tick, 80);
    assert_eq!(profile.gravity_per_tick, 130);
    assert_eq!(profile.fall_speed_per_tick, 2_900);
    assert_eq!(profile.fast_fall_speed_per_tick, 3_500);
    assert_eq!(profile.full_hop_height, 38_520);
    assert_eq!(profile.short_hop_height, 14_850);
    assert_eq!(profile.double_jump_height, 28_560);
    assert_eq!(profile.standing_height_units, 22_667);
    assert_eq!(profile.jumpsquat_frames, 4);
    assert_eq!(profile.dash_frames, 15);
}

#[test]
fn airborne_falcon_profile_uses_profile_gravity_and_fall_speed() {
    let mut world = World::for_two_players();
    step_world(&mut world, Frame(0), &[PlayerInput::neutral().with_jump(true), PlayerInput::neutral()]);
    for frame in 1..=4 {
        step_world(&mut world, Frame(frame), &[PlayerInput::neutral(), PlayerInput::neutral()]);
    }
    let before = world.players()[0].velocity.y;
    step_world(&mut world, Frame(5), &[PlayerInput::neutral(), PlayerInput::neutral()]);
    assert_eq!(world.players()[0].velocity.y, (before - FighterProfile::falcon_like().gravity_per_tick).max(-FighterProfile::falcon_like().fall_speed_per_tick));
}
```

- [ ] **Step 2: Verify RED**

Run: `cargo test -p mole_core falcon_like_profile_exposes_public_falcon_gameplay_values airborne_falcon_profile_uses_profile_gravity_and_fall_speed`

Expected: fails because the profile fields do not exist and sim still uses local constants.

- [ ] **Step 3: Expand `FighterProfile` and wire sim**

Add Falcon-modeled fields with values documented in `docs/research/melee-input-state-reference.md`. Keep existing walk fields and add fields for run, dash, traction, gravity, fall, fast fall, jump heights, standing visual height, sprite reference size, and dash/jumpsquat frame counts.

Replace the sim constants covered by the new profile fields:

```rust
player.velocity.y = (player.velocity.y - player.profile.gravity_per_tick)
    .max(-player.profile.fall_speed_per_tick);
```

For fast fall, clamp to `-player.profile.fast_fall_speed_per_tick`. For dash entry, use `player.profile.initial_dash_speed_per_tick`. For run acceleration clamps, use `player.profile.run_speed_per_tick`.

- [ ] **Step 4: Verify GREEN**

Run: `cargo test -p mole_core falcon_like_profile_exposes_public_falcon_gameplay_values airborne_falcon_profile_uses_profile_gravity_and_fall_speed`

Expected: pass.

- [ ] **Step 5: Run focused movement contracts**

Run: `cargo test -p mole_core wait_soft_stick_enters_walk_slow wait_medium_stick_enters_walk_middle wait_hard_walk_stick_enters_walk_fast dash_threshold_is_separate_from_walk_fast_threshold`

Expected: pass; walk bucket behavior remains intact.

- [ ] **Step 6: Commit**

Run:

```bash
git add crates/mole_core/src/state.rs crates/mole_core/src/sim.rs crates/mole_core/tests/core_contract.rs
git commit -m "feat: use falcon profile physics values"
```

---

### Task 3: Runtime Render Transform, Sprite Scale, and ECB Scene Data

**Files:**
- Modify: `crates/mole_runtime/src/assets.rs`
- Modify: `crates/mole_runtime/src/lib.rs`
- Test: `crates/mole_runtime/tests/runtime_contract.rs`

- [ ] **Step 1: Write failing runtime render tests**

Add tests named:

```rust
#[test]
fn render_transform_maps_core_units_to_screen_without_hidden_gameplay_scale() {
    let transform = RenderTransform::battlefield_camera(960, 540);
    let origin = transform.world_to_screen(Vec2 { x: 0, y: 0 });
    assert_eq!(origin.y, transform.ground_y);
    assert_eq!(transform.pixels_per_core_unit_milli > 0, true);
}

#[test]
fn render_scene_contains_battlefield_surfaces_and_diamond_ecb() {
    let world = World::for_two_players();
    let frame = RenderFrame::from_world(&world);
    let scene = RenderScene::from_frame(&frame, 960, 540);
    assert_eq!(scene.stage_surfaces.len(), 4);
    assert_eq!(scene.player_ecbs[0].points.len(), 4);
    assert_eq!(scene.player_ecbs[0].points[0].x, scene.player_ecbs[0].points[2].x);
}

#[test]
fn dolphin_mole_sprite_scale_uses_standing_height_reference() {
    let visual = DolphinMoleVisualProfile::default();
    assert_eq!(visual.standing_source_height_px, 136);
    assert_eq!(visual.standing_target_height_units, FighterProfile::falcon_like().standing_height_units);
    assert_eq!(visual.scale_milli_for_source_height(136), 167);
    assert_eq!(visual.scaled_size_units(171, 49).height, 8_167);
}
```

- [ ] **Step 2: Verify RED**

Run: `cargo test -p mole_runtime render_transform_maps_core_units_to_screen_without_hidden_gameplay_scale render_scene_contains_battlefield_surfaces_and_diamond_ecb dolphin_mole_sprite_scale_uses_standing_height_reference`

Expected: fails because render transform, stage surfaces, visual profile, and ECB scene data do not exist.

- [ ] **Step 3: Implement runtime scene data**

Add `RenderTransform`, `RenderPoint`, `RenderLine`, `RenderPolygon`, `DolphinMoleVisualProfile`, and sprite source sizes in `assets.rs` or `lib.rs`. Make `RenderScene` contain:

```rust
pub stage_surfaces: Vec<RenderRect>,
pub player_ecbs: [RenderPolygon; 2],
pub player_sprites: [LegacySpriteCue; 2],
```

Keep `players` rectangle fallback during this checkpoint, but derive its size from the visual profile rather than `PLAYER_RENDER_WIDTH`/`PLAYER_RENDER_HEIGHT`.

- [ ] **Step 4: Verify GREEN**

Run: `cargo test -p mole_runtime render_transform_maps_core_units_to_screen_without_hidden_gameplay_scale render_scene_contains_battlefield_surfaces_and_diamond_ecb dolphin_mole_sprite_scale_uses_standing_height_reference`

Expected: pass.

- [ ] **Step 5: Commit**

Run:

```bash
git add crates/mole_runtime/src/assets.rs crates/mole_runtime/src/lib.rs crates/mole_runtime/tests/runtime_contract.rs
git commit -m "feat: render melee unit stage sprite and ecb data"
```

---

### Task 4: SDL Drawing and Frame Logs

**Files:**
- Modify: `crates/mole_runtime/src/lib.rs`
- Modify: `crates/mole_runtime/src/main.rs`
- Test: `crates/mole_runtime/tests/runtime_contract.rs`

- [ ] **Step 1: Write failing logging test**

Add:

```rust
#[test]
fn frame_debug_log_reports_input_state_physics_ecb_and_render_transform() {
    let mut world = World::for_two_players();
    let inputs = [PlayerInput::neutral().with_left_stick(64, 0), PlayerInput::neutral()];
    step_world(&mut world, Frame(0), &inputs);
    let frame = RenderFrame::from_world(&world);
    let scene = RenderScene::from_frame(&frame, 960, 540);
    let line = FrameDebugLog::from_frame_and_scene(&frame, &scene, inputs).to_json_line();
    assert!(line.contains("\"frame\":1"));
    assert!(line.contains("\"p1_bits\":"));
    assert!(line.contains("\"motion_state\":\"WalkMiddle\""));
    assert!(line.contains("\"velocity_x\":"));
    assert!(line.contains("\"ecb\":["));
    assert!(line.contains("\"render_transform\":"));
}
```

- [ ] **Step 2: Verify RED**

Run: `cargo test -p mole_runtime frame_debug_log_reports_input_state_physics_ecb_and_render_transform`

Expected: fails because `FrameDebugLog` does not exist.

- [ ] **Step 3: Implement debug log and SDL drawing**

Add `FrameDebugLog` as a string-producing helper with stable JSON field names. Add `--frame-log` CLI flag that prints one JSON line per simulated frame. Update SDL drawing to draw all `stage_surfaces`, then fallback player rects, then ECB diamond outlines. Keep bitmap sprite loading for the next checkpoint unless SDL image loading is already available without new dependency risk.

- [ ] **Step 4: Verify GREEN**

Run: `cargo test -p mole_runtime frame_debug_log_reports_input_state_physics_ecb_and_render_transform`

Expected: pass.

- [ ] **Step 5: Run SDL smoke command**

Run: `cargo run -p mole_runtime --features "sdl wup" -- --sdl --frames 2 --frame-log`

Expected: exits after two frames, prints `input_backend=wup`, and emits frame log lines.

- [ ] **Step 6: Commit**

Run:

```bash
git add crates/mole_runtime/src/lib.rs crates/mole_runtime/src/main.rs crates/mole_runtime/tests/runtime_contract.rs
git commit -m "feat: draw ecb overlay and frame logs"
```

---

### Task 5: Full Verification and Push

**Files:**
- Verify only unless failures require a focused fix.

- [ ] **Step 1: Run workspace tests**

Run: `cargo test --workspace`

Expected: pass.

- [ ] **Step 2: Run SDL/WUP tests**

Run: `cargo test -p mole_runtime --features "sdl wup"`

Expected: pass.

- [ ] **Step 3: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features -- -D warnings`

Expected: pass with no warnings.

- [ ] **Step 4: Confirm no unsafe blocks in project crates**

Run: `rg -n "unsafe\\s*\\{" crates`

Expected: no matches.

- [ ] **Step 5: Push branch**

Run:

```bash
git status --short --branch
git push origin handoff/rust-rollback-architecture
```

Expected: branch pushes cleanly to `Robbiemas/First_Game`.
