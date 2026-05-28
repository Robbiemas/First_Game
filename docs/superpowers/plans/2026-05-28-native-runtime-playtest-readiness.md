# Native Runtime Playtest Readiness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the native Rust runtime easier to test locally while preserving the legacy visual assets and state graph viewer as explicit Rust migration inputs.

**Architecture:** Rust remains authoritative for simulation, input facts, replay, rollback, and render snapshots. This plan does not move mechanics back into Pygame and does not add a full sprite renderer yet; it adds a Rust-side visual asset manifest and render cues so the SDL renderer can migrate from rectangles to assets in a controlled later pass. The state graph viewer remains a standalone reference/tuning tool, not a gameplay dependency.

**Tech Stack:** Rust stable, Cargo workspace, `mole_runtime`, SDL3 runtime feature, existing Python/Pygame harness tests for legacy launcher status, existing `tools/state_graph_viewer.py`.

---

## Scope

This plan deliberately stops before full PNG texture rendering. The checkpoint is:

- the current DolphinMole art/state folders are inventoried in Rust and tested for existence
- core `MotionState` values map to legacy animation keys through a Rust-owned render cue
- `RenderScene` exposes those cues beside the current deterministic rectangle fallback
- the state graph viewer is documented as a Rust tuning/reference artifact
- the README gives the user a current native playtest command checklist

Environmental collision parity is a required later Rust milestone, but it is out
of scope for this playtest-readiness slice. When that work starts, stage
geometry, platform collision, ECB/contact handling, and ledge/cliff behavior
should be migrated from the Pygame prototype into deterministic Rust systems
with Melee decomp-shaped behavior as the reference.

## File Structure

- Create: `crates/mole_runtime/src/assets.rs`
  - Owns the legacy visual asset manifest.
  - Maps Rust `MotionState` to legacy animation keys.
  - Chooses deterministic frame paths from `motion_state` plus `state_frame`.
- Modify: `crates/mole_runtime/src/lib.rs`
  - Exports the new asset cue types.
  - Adds sprite cues to `RenderScene` without removing rectangle fallback drawing.
- Modify: `crates/mole_runtime/tests/runtime_contract.rs`
  - Tests asset path inventory, state mapping, frame selection, and render scene cues.
- Create: `docs/architecture/visual-asset-and-state-graph-migration.md`
  - Records which legacy assets and graph files are to be repurposed.
  - States that these are visual/tuning references, not authoritative mechanics.
- Modify: `README.md`
  - Adds a short native playtest checklist with Rust runtime, SDL runtime, WUP check, WUP monitor, replay capture, and state graph viewer commands.
- Optional modify only if needed: `execs/README.md`
  - Keep launcher descriptions aligned with the README.
- Verify: `execs/Monitor WUP Native.cmd`
  - The external native WUP input visualizer must run before this checkpoint is considered wrapped.

## Task 1: Add Rust Visual Asset Manifest

**Files:**

- Create: `crates/mole_runtime/src/assets.rs`
- Modify: `crates/mole_runtime/src/lib.rs`
- Test: `crates/mole_runtime/tests/runtime_contract.rs`

- [x] **Step 1: Write failing asset inventory tests**

Add this import block to `crates/mole_runtime/tests/runtime_contract.rs`:

```rust
use std::path::Path;
```

Extend the existing `use mole_runtime::{ ... }` list with:

```rust
    legacy_animation_for_motion_state, LegacyAnimationKey, LegacySpriteCue,
    LEGACY_DOLPHIN_MOLE_ANIMATIONS,
```

Add these tests near the render tests:

```rust
#[test]
fn legacy_dolphin_mole_asset_manifest_points_to_existing_files() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");

    for animation in LEGACY_DOLPHIN_MOLE_ANIMATIONS {
        assert!(
            repo_root.join(animation.directory).is_dir(),
            "missing animation directory {}",
            animation.directory
        );

        for frame in animation.frames {
            let path = repo_root.join(animation.directory).join(frame);
            assert!(path.is_file(), "missing animation frame {}", path.display());
        }
    }
}

#[test]
fn render_asset_mapping_uses_legacy_animation_identity_without_mechanics() {
    assert_eq!(
        legacy_animation_for_motion_state(MotionState::Wait),
        LegacyAnimationKey::Standing
    );
    assert_eq!(
        legacy_animation_for_motion_state(MotionState::WalkSlow),
        LegacyAnimationKey::Walking
    );
    assert_eq!(
        legacy_animation_for_motion_state(MotionState::WalkMiddle),
        LegacyAnimationKey::Walking
    );
    assert_eq!(
        legacy_animation_for_motion_state(MotionState::WalkFast),
        LegacyAnimationKey::Walking
    );
    assert_eq!(
        legacy_animation_for_motion_state(MotionState::Dash),
        LegacyAnimationKey::Dashing
    );
    assert_eq!(
        legacy_animation_for_motion_state(MotionState::Guard),
        LegacyAnimationKey::Blocking
    );
    assert_eq!(
        legacy_animation_for_motion_state(MotionState::EscapeAir),
        LegacyAnimationKey::AirDodge
    );
}

#[test]
fn legacy_sprite_cue_uses_state_frame_and_facing_deterministically() {
    let cue = LegacySpriteCue::for_player(MotionState::Wait, 3, -1);

    assert_eq!(cue.animation, LegacyAnimationKey::Standing);
    assert_eq!(cue.directory, "DolphinMole/standing");
    assert_eq!(cue.frame, "Standing2.png");
    assert!(cue.flip_x);
}
```

- [x] **Step 2: Run tests to verify they fail**

Run:

```powershell
cargo test -p mole_runtime legacy_dolphin_mole_asset_manifest_points_to_existing_files
cargo test -p mole_runtime render_asset_mapping_uses_legacy_animation_identity_without_mechanics
cargo test -p mole_runtime legacy_sprite_cue_uses_state_frame_and_facing_deterministically
```

Expected: FAIL because `assets.rs`, `LegacyAnimationKey`, `LegacySpriteCue`, and `LEGACY_DOLPHIN_MOLE_ANIMATIONS` do not exist yet.

- [x] **Step 3: Add the asset manifest implementation**

Create `crates/mole_runtime/src/assets.rs`:

```rust
use mole_core::MotionState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyAnimationKey {
    Standing,
    Running,
    Dashing,
    Walking,
    Air,
    LandingLag,
    AirDodge,
    JumpSquat,
    FreeFall,
    Turning,
    RunTurn,
    Blocking,
    Shield,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegacyAnimationSpec {
    pub key: LegacyAnimationKey,
    pub directory: &'static str,
    pub frames: &'static [&'static str],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegacySpriteCue {
    pub animation: LegacyAnimationKey,
    pub directory: &'static str,
    pub frame: &'static str,
    pub flip_x: bool,
}

pub const LEGACY_DOLPHIN_MOLE_ANIMATIONS: &[LegacyAnimationSpec] = &[
    LegacyAnimationSpec {
        key: LegacyAnimationKey::Standing,
        directory: "DolphinMole/standing",
        frames: &["Standing1.png", "Standing2.png"],
    },
    LegacyAnimationSpec {
        key: LegacyAnimationKey::Running,
        directory: "DolphinMole/running",
        frames: &["running1 - Copy.png", "running1.png"],
    },
    LegacyAnimationSpec {
        key: LegacyAnimationKey::Dashing,
        directory: "DolphinMole/dashing",
        frames: &["running1 - Copy.png", "running1.png"],
    },
    LegacyAnimationSpec {
        key: LegacyAnimationKey::Walking,
        directory: "DolphinMole/walking",
        frames: &["Standing1.png", "Standing2.png"],
    },
    LegacyAnimationSpec {
        key: LegacyAnimationKey::Air,
        directory: "DolphinMole/air",
        frames: &["Air1 - Copy.png", "Air1.png"],
    },
    LegacyAnimationSpec {
        key: LegacyAnimationKey::LandingLag,
        directory: "DolphinMole/landingLag",
        frames: &["running1 - Copy.png", "running1.png"],
    },
    LegacyAnimationSpec {
        key: LegacyAnimationKey::AirDodge,
        directory: "DolphinMole/airDodge",
        frames: &["running1 - Copy.png"],
    },
    LegacyAnimationSpec {
        key: LegacyAnimationKey::JumpSquat,
        directory: "DolphinMole/jumpSquat",
        frames: &["Standing1.png", "Standing2.png"],
    },
    LegacyAnimationSpec {
        key: LegacyAnimationKey::FreeFall,
        directory: "DolphinMole/freeFall",
        frames: &["running1 - Copy.png"],
    },
    LegacyAnimationSpec {
        key: LegacyAnimationKey::Turning,
        directory: "DolphinMole/turning",
        frames: &["Standing1.png"],
    },
    LegacyAnimationSpec {
        key: LegacyAnimationKey::RunTurn,
        directory: "DolphinMole/runTurn",
        frames: &["Standing1.png"],
    },
    LegacyAnimationSpec {
        key: LegacyAnimationKey::Blocking,
        directory: "DolphinMole/blocking",
        frames: &["Standing1.png"],
    },
    LegacyAnimationSpec {
        key: LegacyAnimationKey::Shield,
        directory: "DolphinMole/shield",
        frames: &["shield.png"],
    },
];

impl LegacySpriteCue {
    pub fn for_player(motion_state: MotionState, state_frame: u8, facing: i8) -> Self {
        let animation = legacy_animation_for_motion_state(motion_state);
        let spec = legacy_animation_spec(animation);
        let frame_index = state_frame as usize % spec.frames.len();

        Self {
            animation,
            directory: spec.directory,
            frame: spec.frames[frame_index],
            flip_x: facing < 0,
        }
    }
}

pub fn legacy_animation_for_motion_state(motion_state: MotionState) -> LegacyAnimationKey {
    match motion_state {
        MotionState::Wait
        | MotionState::Attack1
        | MotionState::AttackS3
        | MotionState::AttackHi3
        | MotionState::AttackLw3
        | MotionState::AttackS4
        | MotionState::AttackHi4
        | MotionState::AttackLw4
        | MotionState::SpecialN
        | MotionState::SpecialS
        | MotionState::SpecialHi
        | MotionState::SpecialLw
        | MotionState::Catch
        | MotionState::CatchDash
        | MotionState::Squat => LegacyAnimationKey::Standing,
        MotionState::WalkSlow | MotionState::WalkMiddle | MotionState::WalkFast => {
            LegacyAnimationKey::Walking
        }
        MotionState::Dash => LegacyAnimationKey::Dashing,
        MotionState::Run | MotionState::RunBrake | MotionState::AttackDash => {
            LegacyAnimationKey::Running
        }
        MotionState::Turn => LegacyAnimationKey::Turning,
        MotionState::TurnRun => LegacyAnimationKey::RunTurn,
        MotionState::KneeBend => LegacyAnimationKey::JumpSquat,
        MotionState::JumpF
        | MotionState::JumpB
        | MotionState::JumpAerialF
        | MotionState::JumpAerialB
        | MotionState::Air
        | MotionState::AttackAirN
        | MotionState::AttackAirF
        | MotionState::AttackAirB
        | MotionState::AttackAirHi
        | MotionState::AttackAirLw
        | MotionState::SpecialAirN
        | MotionState::SpecialAirS
        | MotionState::SpecialAirHi
        | MotionState::SpecialAirLw => LegacyAnimationKey::Air,
        MotionState::GuardOn | MotionState::Guard | MotionState::GuardOff => {
            LegacyAnimationKey::Blocking
        }
        MotionState::EscapeN | MotionState::EscapeF | MotionState::EscapeB => {
            LegacyAnimationKey::Dashing
        }
        MotionState::EscapeAir => LegacyAnimationKey::AirDodge,
        MotionState::FallSpecial => LegacyAnimationKey::FreeFall,
        MotionState::Landing | MotionState::LandingFallSpecial => LegacyAnimationKey::LandingLag,
    }
}

pub fn legacy_animation_spec(key: LegacyAnimationKey) -> &'static LegacyAnimationSpec {
    LEGACY_DOLPHIN_MOLE_ANIMATIONS
        .iter()
        .find(|spec| spec.key == key)
        .expect("legacy animation key is covered by manifest")
}
```

In `crates/mole_runtime/src/lib.rs`, add:

```rust
pub mod assets;
pub use assets::{
    legacy_animation_for_motion_state, legacy_animation_spec, LegacyAnimationKey,
    LegacyAnimationSpec, LegacySpriteCue, LEGACY_DOLPHIN_MOLE_ANIMATIONS,
};
```

- [x] **Step 4: Run asset tests to verify they pass**

Run:

```powershell
cargo test -p mole_runtime legacy_dolphin_mole_asset_manifest_points_to_existing_files
cargo test -p mole_runtime render_asset_mapping_uses_legacy_animation_identity_without_mechanics
cargo test -p mole_runtime legacy_sprite_cue_uses_state_frame_and_facing_deterministically
```

Expected: PASS.

## Task 2: Attach Sprite Cues To RenderScene

**Files:**

- Modify: `crates/mole_runtime/src/lib.rs`
- Test: `crates/mole_runtime/tests/runtime_contract.rs`

- [x] **Step 1: Write failing render cue test**

Add this test near `render_scene_places_players_deterministically_from_render_frame`:

```rust
#[test]
fn render_scene_exposes_legacy_sprite_cues_without_replacing_rect_fallback() {
    let mut world = World::for_two_players();
    step_world(
        &mut world,
        Frame(0),
        &[PlayerInput::neutral().with_left_stick(64, 0), PlayerInput::neutral()],
    );
    let render_frame = RenderFrame::from_world(&world);

    let scene = RenderScene::from_frame(&render_frame, 960, 540);

    assert_eq!(scene.player_sprites[0].animation, LegacyAnimationKey::Walking);
    assert_eq!(scene.player_sprites[0].directory, "DolphinMole/walking");
    assert!(!scene.player_sprites[0].flip_x);
    assert_eq!(scene.player_sprites[1].animation, LegacyAnimationKey::Standing);
    assert!(scene.player_sprites[1].flip_x);
    assert_eq!(scene.players[0].width, 48);
    assert_eq!(scene.players[0].height, 72);
}
```

- [x] **Step 2: Run test to verify it fails**

Run:

```powershell
cargo test -p mole_runtime render_scene_exposes_legacy_sprite_cues_without_replacing_rect_fallback
```

Expected: FAIL because `RenderScene::player_sprites` does not exist yet.

- [x] **Step 3: Add sprite cues to the scene**

In `crates/mole_runtime/src/lib.rs`, update `RenderScene`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderScene {
    pub background: RenderColor,
    pub stage: RenderRect,
    pub players: [RenderRect; 2],
    pub player_sprites: [LegacySpriteCue; 2],
}
```

Inside `RenderScene::from_frame`, add the `player_sprites` field:

```rust
            player_sprites: [
                LegacySpriteCue::for_player(
                    frame.player_motion_states[0],
                    frame.player_state_frames[0],
                    frame.player_facings[0],
                ),
                LegacySpriteCue::for_player(
                    frame.player_motion_states[1],
                    frame.player_state_frames[1],
                    frame.player_facings[1],
                ),
            ],
```

Update the existing `render_scene_places_players_deterministically_from_render_frame` expected `RenderScene` shape by asserting only fields that should stay stable, or by adding the exact `player_sprites` values for both starting players.

- [x] **Step 4: Run runtime tests**

Run:

```powershell
cargo test -p mole_runtime
```

Expected: PASS.

## Task 3: Document Visual And State Graph Migration Inputs

**Files:**

- Create: `docs/architecture/visual-asset-and-state-graph-migration.md`
- Modify: `docs/architecture/native-rust-rollback-architecture.md`

- [x] **Step 1: Create the migration note**

Create `docs/architecture/visual-asset-and-state-graph-migration.md`:

```markdown
# Visual Asset And State Graph Migration

The Rust runtime is the authoritative gameplay path. The legacy Pygame project
still contains visual and tuning artifacts that should be preserved and migrated
deliberately.

## Reusable Visual Assets

- `background.png`
- `DolphinMole/standing/*.png`
- `DolphinMole/walking/*.png`
- `DolphinMole/dashing/*.png`
- `DolphinMole/running/*.png`
- `DolphinMole/landingLag/*.png`
- `DolphinMole/airDodge/*.png`
- `DolphinMole/jumpSquat/*.png`
- `DolphinMole/freeFall/*.png`
- `DolphinMole/turning/*.png`
- `DolphinMole/runTurn/*.png`
- `DolphinMole/blocking/*.png`
- `DolphinMole/shield/*.png`

The Rust runtime manifest in `crates/mole_runtime/src/assets.rs` maps Rust
`MotionState` values to these legacy animation groups. That manifest is render
metadata only; mechanics remain in `mole_core`.

## State Graph Viewer

- `tools/state_graph_viewer.py`
- `docs/state_graphs/melee_reference_graph.json`
- `docs/state_graphs/mole_current_graph.json`
- `config/state_graph_layout.json`
- `execs/Open State Graphs.cmd`

Keep this viewer as a side-by-side Melee/Mole state transition reference and
tuning aid. It should help humans inspect state-shape decisions, but it should
not become an authoritative runtime dependency.
```

- [x] **Step 2: Link the note from the architecture document**

In `docs/architecture/native-rust-rollback-architecture.md`, add one sentence near the asset preservation note:

```markdown
See [Visual Asset And State Graph Migration](./visual-asset-and-state-graph-migration.md)
for the current preserved asset and state graph inventory.
```

- [x] **Step 3: Run doc checks**

Run:

```powershell
rg -n "Visual Asset And State Graph Migration|assets.rs|state_graph_viewer" docs/architecture docs/superpowers/plans
```

Expected: the new document and links are found.

## Task 4: Add Native Playtest Checklist To README

**Files:**

- Modify: `README.md`
- Optional modify: `execs/README.md`

- [x] **Step 1: Add README checklist**

Add this section after `## Rust Rollback Path` setup commands:

```markdown
## Native Rust Playtest Checklist

From `D:\Mole Game\First_Game`:

```powershell
cargo test --workspace
cargo run -p mole_runtime -- --frames 120
powershell -NoProfile -ExecutionPolicy Bypass -File tools\setup_sdl3.ps1
cargo run -p mole_runtime --features "sdl wup" -- --sdl --frames 600
cargo run -p mole_runtime -- --record-replay --frames 600
```

For GameCube/WUP checks:

```powershell
.\execs\Check WUP Native.cmd
.\execs\Monitor WUP Native.cmd
.\execs\Run WUP Native Runtime.cmd
```

For visual state reference:

```powershell
.\execs\Open State Graphs.cmd
```

The SDL runtime currently uses deterministic rectangle fallback rendering plus
Rust-owned sprite cues. The checked-in DolphinMole assets are preserved and
mapped in Rust for the next renderer pass.
```

- [x] **Step 2: Verify launcher names are still current**

Run:

```powershell
Get-ChildItem execs -Filter *.cmd | Select-Object -ExpandProperty Name
```

Expected: every command listed in the README exists.

## Task 5: Verify Native WUP Input Visualizer

**Files:**

- Read: `execs/Monitor WUP Native.cmd`
- Read: `crates/mole_runtime/src/wup_monitor.rs`
- Verify: native monitor command

- [x] **Step 1: Confirm the launcher targets the Rust monitor path**

Run:

```powershell
Get-Content -LiteralPath 'execs\Monitor WUP Native.cmd'
```

Expected: it runs:

```cmd
cargo run -p mole_runtime --features "sdl wup" -- --monitor-wup
```

- [x] **Step 2: Smoke-run the monitor with a frame limit**

Run:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tools\setup_sdl3.ps1
cargo run -p mole_runtime --features "sdl wup" -- --monitor-wup --frames 2
```

Expected with WUP-028 adapter available through WinUSB/libusb: an SDL window titled `Mole WUP-028 Native Input Monitor` opens, polls native WUP input, renders the external visualizer, exits after two frames, and returns exit code `0`.

Expected without adapter access: the command reports the WUP open/read error clearly. In that case, do not mark the whole branch wrapped for hardware input until the user can run the same command with the adapter connected.

- [x] **Step 3: Keep monitor diagnostics separate from gameplay authority**

Confirm by inspection that `crates/mole_runtime/src/wup_monitor.rs` uses `InputReadout::from_wup_ports_with_melee_snapshots(...)` for display and does not feed monitor state back into `mole_core::World`.

## Task 6: Verification And Commit

**Files:**

- Verify all modified files.

- [x] **Step 1: Format**

Run:

```powershell
cargo fmt --all -- --check
```

Expected: PASS.

- [x] **Step 2: Rust tests**

Run:

```powershell
cargo test --workspace
cargo test -p mole_transport --features webrtc
```

Expected: PASS.

- [x] **Step 3: Python tests**

Run:

```powershell
.\.venv\Scripts\python.exe -m pytest
```

Expected: PASS.

- [x] **Step 4: Lints and safety scan**

Run:

```powershell
cargo clippy --workspace --all-targets
rg -n "\bunsafe\b" -g "*.rs" crates
```

Expected: clippy has no warnings; `rg` finds no first-party `unsafe` blocks under `crates`.

- [x] **Step 5: Commit and push**

Run:

```powershell
git status --short --branch
git add crates/mole_runtime/src/assets.rs crates/mole_runtime/src/lib.rs crates/mole_runtime/tests/runtime_contract.rs docs/architecture/visual-asset-and-state-graph-migration.md docs/architecture/native-rust-rollback-architecture.md README.md execs/README.md docs/superpowers/plans/2026-05-28-native-runtime-playtest-readiness.md
git commit -m "feat: prepare native runtime playtest assets"
git push origin handoff/rust-rollback-architecture
```

Expected: push succeeds and the branch is clean afterward.

## Self-Review

- Spec coverage: This plan covers the user-testable native Rust direction, asset preservation, state graph preservation, the external WUP input visualizer, and WUP/UCF staying in Rust. It does not implement combat, full PNG texture drawing, or Pygame mechanics.
- Placeholder scan: no `TBD`, `TODO`, or underspecified edge-handling steps are present.
- Type consistency: `LegacyAnimationKey`, `LegacyAnimationSpec`, `LegacySpriteCue`, and `RenderScene::player_sprites` are introduced before later tasks reference them.
