# Melee 3D Collision Sampler Handoff And Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace static per-frame hitbox/hurtbox materialization with a Rust 3D JObj/FigaTree pose sampler that emits Melee-shaped hit and hurt capsules for dev-tool display and runtime collision.

**Architecture:** The compact manifest remains canonical. It stores source action records, decoded subaction procedures, `ftData.x30` hurtbox init data, skeleton/JObj data, and source provenance. The Rust backend samples Melee XYZ pose and capsules on demand, runs capsule-first 3D collision, and treats 2D projection as a view/runtime compatibility layer only.

**Tech Stack:** Rust `mole_core`, `mole_cli`, `mole_runtime`, SDL debug rendering, JSON source artifacts under `resources/melee`, local Melee decomp under `.research/doldecomp-melee`.

---

## Handoff Context

Repository proof expected at start of execution:

- Workspace: `D:\Mole Game\First_Game`
- Remote: `https://github.com/Robbiemas/First_Game.git`
- Current continuity branch: `handoff/rust-rollback-architecture`
- Current tree is dirty from ongoing frame-data/runtime work. Do not revert unrelated changes.
- Do not commit or push unless the user explicitly asks.

Current important artifacts:

- `resources/melee/frame_data/dolphin_mole/source_manifest.json`
  - Compact canonical import.
  - Current count observed: 275 source actions, 65 runtime-mapped actions, 210 Rust parity gaps, 11 Captain Falcon hurtbox init records, 63 skeleton joints, 302 decoded procedures.
- `resources/melee/frame_data/dolphin_mole/AttackAirN.json`
  - Expanded derived Nair view used by the current dev tool/runtime proof.
- `crates/mole_runtime/src/generated/frame_data_boxes.rs`
  - Temporary generated static capsule table for Nair only. This is a proof path, not the full-character architecture.
- `resources/melee/extracted/captain_falcon_action_hurtbox_samples.json`
  - Large derived debug cache, about 164 MB. Treat this as evidence that all-state per-frame materialization is the wrong canonical format.

Do not re-argue the architecture unless decomp evidence contradicts it. The chosen direction is 3D-first, capsule-first, Melee-shaped backend sampling.

## Source Anchors

Before changing code, inspect these anchors and cite them in comments/tests where appropriate:

- `.research/doldecomp-melee/src/melee/ft/ftaction.c`
  - `ftAction_8007121C`: creates fighter hit capsules from action commands.
  - `ftAction_80071A9C`: applies per-bone hurt capsule state changes.
  - `ftAction_80073240`: advances/executes fighter command script events against current animation frame.
- `.research/doldecomp-melee/src/melee/ft/ftcoll.c`
  - `ftColl_8007AD18`: updates hit capsule current/previous endpoints by transforming local offsets through JObj.
  - `ftColl_8007B320`: initializes fighter hurt capsules from `fp->ft_data->x30`.
  - `ftColl_8007B4E0`: refreshes hurt capsules and marks positions dirty.
  - `ftColl_8007B0C0` and `ftColl_8007B128`: set all/one hurt capsule state.
- `.research/doldecomp-melee/src/melee/lb/lbcollision.c`
  - `lbColl_80006E58`: 3D capsule-vs-capsule collision primitive.
  - `lbColl_800083C4`: transforms hurt capsule `a_offset`/`b_offset` into `a_pos`/`b_pos`.
  - `lbColl_8000A244` and `lbColl_8000A584`: debug draw/update path for hurt capsules.
- `.research/doldecomp-melee/src/melee/lb/types.h`
  - `struct HitCapsule`, `struct HurtCapsule`, `struct FighterHurtCapsule`.
- `.research/doldecomp-melee/src/melee/ft/chara/ftCommon/types.h`
  - `struct ftHurtboxInit`.
- `.research/doldecomp-melee/src/melee/lb/lbanim.h`
  - `struct FigaTree`, `struct FigaTrack`.
- `.research/doldecomp-melee/src/sysdolphin/baselib/jobj.c` and `jobj.h`
  - `HSD_JObjReqAnimAll`, `HSD_JObjAnimAll`, `HSD_JObjSetupMatrix`, `HSD_JObjLoadJoint`.

## File Structure

Keep the backend engine-agnostic and avoid a Falcon-only path:

- Create: `crates/mole_core/src/collision.rs`
  - Pure Rust source-space math and capsule data types.
  - No `serde`, no file I/O, no runtime rendering.
- Modify: `crates/mole_core/src/lib.rs`
  - Export the collision module.
- Create: `crates/mole_core/tests/collision_contract.rs`
  - Unit tests for pure math and capsule state behavior.
- Create: `crates/mole_cli/src/frame_data_sampler.rs`
  - JSON manifest loader plus source-action sampler bridge.
  - Allowed to use `serde` and `serde_json`.
- Modify: `crates/mole_cli/src/lib.rs`
  - Add `frame-data sample` command parsing and report routing.
- Modify: `crates/mole_cli/src/frame_data.rs`
  - Reuse existing manifest helpers where possible.
  - Keep `extract --all-states` compact.
  - Keep legacy `export-runtime --state` working until runtime fully moves to the sampler.
- Modify: `crates/mole_cli/tests/cli_contract.rs`
  - Add CLI contract tests for manifest-backed sampling.
- Modify: `tools/state_graph_viewer.py`
  - Feed manifest-only states through `frame-data sample` instead of showing only "No keyframes".
  - Preserve current Nair display behavior while adding sampler path.
- Modify: `tests/test_state_graph_viewer.py`
  - Prove manifest-only states populate sampled capsule details when the CLI sampler returns data.
- Modify: `crates/mole_runtime/src/lib.rs`
  - Replace or wrap the static Nair debug capsule path with a sampler-backed path once the sampler is ready.
- Modify: `crates/mole_runtime/tests/runtime_contract.rs`
  - Prove runtime debug capsules use sampler data and keep red/yellow colors.

## Task 1: Lock Pure Collision Types And 3D Capsule Math

**Files:**
- Create: `crates/mole_core/src/collision.rs`
- Modify: `crates/mole_core/src/lib.rs`
- Test: `crates/mole_core/tests/collision_contract.rs`

- [ ] **Step 1: Write pure capsule math tests**

Add tests that use simple 3D capsules and do not read JSON:

```rust
use mole_core::collision::{capsules_intersect_3d, Capsule3, Vec3};

#[test]
fn capsule_collision_preserves_z_axis_separation() {
    let hit = Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(2.0, 0.0, 0.0), 0.5);
    let hurt = Capsule3::new(Vec3::new(1.0, 0.0, 1.25), Vec3::new(1.0, 2.0, 1.25), 0.5);

    assert!(!capsules_intersect_3d(&hit, &hurt));
}

#[test]
fn capsule_collision_hits_when_3d_distance_is_inside_combined_radius() {
    let hit = Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(2.0, 0.0, 0.0), 0.75);
    let hurt = Capsule3::new(Vec3::new(1.0, 0.0, 0.9), Vec3::new(1.0, 2.0, 0.9), 0.75);

    assert!(capsules_intersect_3d(&hit, &hurt));
}
```

- [ ] **Step 2: Run the failing test**

Run: `cargo test -p mole_core --test collision_contract`

Expected: FAIL because `mole_core::collision` does not exist.

- [ ] **Step 3: Implement source-space types and 3D capsule math**

Create `crates/mole_core/src/collision.rs` with:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Capsule3 {
    pub a: Vec3,
    pub b: Vec3,
    pub radius: f32,
}

impl Capsule3 {
    pub const fn new(a: Vec3, b: Vec3, radius: f32) -> Self {
        Self { a, b, radius }
    }
}

fn dot(a: Vec3, b: Vec3) -> f32 {
    a.x * b.x + a.y * b.y + a.z * b.z
}

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(a.x - b.x, a.y - b.y, a.z - b.z)
}

fn add(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(a.x + b.x, a.y + b.y, a.z + b.z)
}

fn mul(v: Vec3, scalar: f32) -> Vec3 {
    Vec3::new(v.x * scalar, v.y * scalar, v.z * scalar)
}

fn closest_distance_sq_between_segments(p1: Vec3, q1: Vec3, p2: Vec3, q2: Vec3) -> f32 {
    let d1 = sub(q1, p1);
    let d2 = sub(q2, p2);
    let r = sub(p1, p2);
    let a = dot(d1, d1);
    let e = dot(d2, d2);
    let f = dot(d2, r);

    let (mut s, mut t);
    if a <= f32::EPSILON && e <= f32::EPSILON {
        return dot(r, r);
    }
    if a <= f32::EPSILON {
        s = 0.0;
        t = (f / e).clamp(0.0, 1.0);
    } else {
        let c = dot(d1, r);
        if e <= f32::EPSILON {
            t = 0.0;
            s = (-c / a).clamp(0.0, 1.0);
        } else {
            let b = dot(d1, d2);
            let denom = a * e - b * b;
            s = if denom.abs() > f32::EPSILON {
                ((b * f - c * e) / denom).clamp(0.0, 1.0)
            } else {
                0.0
            };
            t = (b * s + f) / e;
            if t < 0.0 {
                t = 0.0;
                s = (-c / a).clamp(0.0, 1.0);
            } else if t > 1.0 {
                t = 1.0;
                s = ((b - c) / a).clamp(0.0, 1.0);
            }
        }
    }

    let c1 = add(p1, mul(d1, s));
    let c2 = add(p2, mul(d2, t));
    dot(sub(c1, c2), sub(c1, c2))
}

pub fn capsules_intersect_3d(hit: &Capsule3, hurt: &Capsule3) -> bool {
    let allowed = hit.radius + hurt.radius;
    closest_distance_sq_between_segments(hit.a, hit.b, hurt.a, hurt.b) <= allowed * allowed
}
```

Add `pub mod collision;` to `crates/mole_core/src/lib.rs`.

- [ ] **Step 4: Verify**

Run: `cargo test -p mole_core --test collision_contract`

Expected: PASS.

## Task 2: Add Source-Space JObj Pose Types And Transform Tests

**Files:**
- Modify: `crates/mole_core/src/collision.rs`
- Test: `crates/mole_core/tests/collision_contract.rs`

- [ ] **Step 1: Write transform tests**

Add tests for a 3x4 matrix transform that preserves source Z:

```rust
use mole_core::collision::{Mat3x4, Vec3};

#[test]
fn mat3x4_transform_point_keeps_source_z() {
    let matrix = Mat3x4::from_rows([
        [1.0, 0.0, 0.0, 10.0],
        [0.0, 1.0, 0.0, 20.0],
        [0.0, 0.0, 1.0, 30.0],
    ]);

    let point = matrix.transform_point(Vec3::new(1.5, 2.5, 3.5));

    assert_eq!(point, Vec3::new(11.5, 22.5, 33.5));
}
```

- [ ] **Step 2: Run the failing test**

Run: `cargo test -p mole_core --test collision_contract mat3x4_transform_point_keeps_source_z`

Expected: FAIL because `Mat3x4` does not exist.

- [ ] **Step 3: Implement matrix type**

Add to `crates/mole_core/src/collision.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mat3x4 {
    pub rows: [[f32; 4]; 3],
}

impl Mat3x4 {
    pub const fn from_rows(rows: [[f32; 4]; 3]) -> Self {
        Self { rows }
    }

    pub fn transform_point(&self, point: Vec3) -> Vec3 {
        Vec3::new(
            self.rows[0][0] * point.x + self.rows[0][1] * point.y + self.rows[0][2] * point.z + self.rows[0][3],
            self.rows[1][0] * point.x + self.rows[1][1] * point.y + self.rows[1][2] * point.z + self.rows[1][3],
            self.rows[2][0] * point.x + self.rows[2][1] * point.y + self.rows[2][2] * point.z + self.rows[2][3],
        )
    }
}
```

- [ ] **Step 4: Verify**

Run: `cargo test -p mole_core --test collision_contract`

Expected: PASS.

## Task 3: Port The Manifest-Backed Sampler Into Rust CLI

**Files:**
- Create: `crates/mole_cli/src/frame_data_sampler.rs`
- Modify: `crates/mole_cli/src/lib.rs`
- Modify: `crates/mole_cli/src/frame_data.rs`
- Test: `crates/mole_cli/tests/cli_contract.rs`

- [ ] **Step 1: Add a failing CLI contract for `frame-data sample`**

Add a test that invokes:

```text
mole frame-data sample --character dolphin_mole --source-character captain --state AttackAirN --frame 7 --json
```

Assert:

- `command == "frame-data sample"`
- `character == "dolphin_mole"`
- `state == "AttackAirN"`
- `source_space == "melee_xyz"`
- hit capsules include source `x`, `y`, and `z`
- hurt capsules include source `x`, `y`, and `z`
- projected capsules are marked as derived debug view

- [ ] **Step 2: Run the failing test**

Run: `cargo test -p mole_cli --test cli_contract frame_data_sample_attack_air_n_uses_compact_manifest`

Expected: FAIL because the command is not parsed.

- [ ] **Step 3: Add command parsing**

Add a `Sample(FrameDataSampleOptions)` command variant and parse:

```text
frame-data sample --character ID --source-character ID --state MotionStateOrSourceAction --frame N [--json]
```

The sample command must not write files.

- [ ] **Step 4: Implement a narrow sampler bridge**

In `frame_data_sampler.rs`, load `resources/melee/frame_data/<character>/source_manifest.json`, resolve the action by `state` or source action key, and produce a report from existing expanded Nair data only as a temporary bridge when `AttackAirN.json` exists.

Important boundary: label that bridge as `artifact_kind: "derived_expanded_state_bridge"`. Do not pretend this is the final JObj/FigaTree runtime sampler.

- [ ] **Step 5: Verify**

Run:

```powershell
cargo test -p mole_cli --test cli_contract frame_data_sample_attack_air_n_uses_compact_manifest
cargo test -p mole_cli --test cli_contract
```

Expected: PASS.

## Task 4: Port JObj/FigaTree Pose Sampling From Python To Rust

**Files:**
- Modify: `crates/mole_core/src/collision.rs`
- Modify: `crates/mole_cli/src/frame_data_sampler.rs`
- Test: `crates/mole_cli/tests/cli_contract.rs`
- Reference: `tools/extract_melee_resources.py`

- [ ] **Step 1: Read the existing Python reference**

Inspect:

- `sample_fobj_value`
- `sample_figatree_node_tracks`
- `sample_figatree_skeleton_pose`
- `sample_hurtboxes_from_pose`

These are reference implementations only. The runtime-facing sampler should be Rust.

- [ ] **Step 2: Add Rust tests against known Nair sample facts**

Use `AttackAirN` frame 7 as the first fixture. Compare one known hurt capsule endpoint and one known hit capsule endpoint against the existing expanded artifact within a small float tolerance.

Expected behavior:

- Source XYZ values are preserved as floats.
- Flattened/projected values are derived and never overwrite source XYZ.

- [ ] **Step 3: Port FObj/FigaTree track decoding**

Implement only the track op/channel coverage already used by Captain Falcon's source manifest. If an unsupported track op appears, return a structured sampler error naming the action, node, track, and op. Do not guess.

- [ ] **Step 4: Port skeleton matrix evaluation**

Evaluate each JObj in source order using bind pose SRT plus sampled track overrides. Produce world matrices equivalent to the Python `sample_figatree_skeleton_pose` output.

- [ ] **Step 5: Verify**

Run:

```powershell
cargo test -p mole_cli --test cli_contract frame_data_sample_attack_air_n_uses_compact_manifest
cargo test -p mole_cli --test cli_contract
```

Expected: PASS with no static expanded artifact required for the sampled values.

## Task 5: Implement Melee Hit And Hurt Capsule State Sampling

**Files:**
- Modify: `crates/mole_core/src/collision.rs`
- Modify: `crates/mole_cli/src/frame_data_sampler.rs`
- Test: `crates/mole_core/tests/collision_contract.rs`
- Test: `crates/mole_cli/tests/cli_contract.rs`

- [ ] **Step 1: Add hit capsule state tests**

Test the Melee state machine shape:

- Newly spawned hit capsule starts as `HitCapsule_Enabled`.
- First update transforms local offset to current endpoint, copies current to previous, then state becomes `HitCapsule_Unk2`.
- Next update transitions to `HitCapsule_Unk3`.
- Subsequent updates copy current to previous before transforming the new current endpoint.

- [ ] **Step 2: Add hurt capsule init tests**

Test that `ftData.x30` records create enabled hurt capsules with:

- `bone_idx`
- `height`
- `is_grabbable`
- `a_offset`
- `b_offset`
- `scale`
- transformed `a_pos` and `b_pos`

- [ ] **Step 3: Apply decoded action commands**

Use decoded procedures from `source_manifest.json`:

- Hitbox spawn/update commands map to `ftAction_8007121C` semantics.
- Hurt-state commands map to `ftAction_80071A9C` semantics.
- Clear/disable commands must follow the decoded decomp command, not inferred timings.

- [ ] **Step 4: Verify Nair frame output**

Run:

```powershell
cargo run -q -p mole_cli -- frame-data sample --character dolphin_mole --source-character captain --state AttackAirN --frame 7 --json
```

Expected:

- Red hit capsule debug records exist for active Nair frames.
- Yellow hurt capsule debug records exist for the sampled pose.
- Both expose source 3D endpoints and projected view endpoints.

## Task 6: Feed The Dev Tool From The Sampler

**Files:**
- Modify: `tools/state_graph_viewer.py`
- Test: `tests/test_state_graph_viewer.py`

- [ ] **Step 1: Add a test for manifest-only state sampling**

Use a state present in `source_manifest.json` but missing an expanded artifact, such as `SpecialN` if still manifest-only.

Assert the dev tool loader can request a frame sample and display:

- source character
- source state/action
- frame
- hit capsule table if any
- hurt capsule table
- source XYZ and projected debug XYZ

- [ ] **Step 2: Preserve the existing right-facing display rule**

The view may project for readability, but the source data remains Melee XYZ. The UI must label source and projected values separately.

- [ ] **Step 3: Verify**

Run:

```powershell
python -m pytest tests/test_state_graph_viewer.py -q
```

Expected: PASS.

## Task 7: Feed Runtime Debug Rendering And Collision From The Sampler

**Files:**
- Modify: `crates/mole_runtime/src/lib.rs`
- Modify: `crates/mole_runtime/tests/runtime_contract.rs`

- [ ] **Step 1: Add runtime contract tests**

Assert:

- Hitboxes render as red semi-translucent capsules.
- Hurtboxes render as yellow semi-translucent capsules.
- Runtime source capsule data still contains Z.
- Any current 2D runtime projection is derived from the source capsule and is not stored as canonical source.

- [ ] **Step 2: Replace the Nair-only generated lookup**

Route current Nair debug capsules through the sampler-backed API. Keep the generated file only if needed as an interim compatibility shim and mark it temporary in comments.

- [ ] **Step 3: Verify**

Run:

```powershell
cargo test -p mole_runtime
```

Expected: PASS.

## Task 8: Full Verification And Handoff

**Files:**
- Modify if needed: `docs/research/melee-data-provenance-audit.md`
- Modify if needed: `docs/superpowers/specs/2026-06-01-move-frame-data-design.md`

- [ ] **Step 1: Run formatting**

Run: `cargo fmt --all`

Expected: no formatting errors.

- [ ] **Step 2: Run focused Rust tests**

Run:

```powershell
cargo test -p mole_core --test collision_contract
cargo test -p mole_cli --test cli_contract
cargo test -p mole_runtime
```

Expected: PASS.

- [ ] **Step 3: Run focused Python dev-tool tests**

Run: `python -m pytest tests/test_state_graph_viewer.py -q`

Expected: PASS. If Python environment fails because of missing dependencies, report the interpreter path and do not mislabel that as a gameplay regression.

- [ ] **Step 4: Record known gaps**

Document:

- Unsupported FigaTree track ops, if any.
- Source actions that still cannot be sampled.
- Rust `MotionState` parity gaps that remain source-only.
- Any runtime use of a temporary generated Nair bridge.

## Ready-To-Paste Execution Prompt

```text
We are continuing work on Robbiemas/First_Game in D:\Mole Game\First_Game.

Repository proof to verify before editing:
- Remote: https://github.com/Robbiemas/First_Game.git
- Branch: handoff/rust-rollback-architecture
- The worktree may be dirty from ongoing frame-data/runtime work. Do not revert unrelated changes.
- Do not commit or push unless I explicitly ask.

Primary directive:
Read and execute docs/superpowers/plans/2026-06-03-melee-3d-collision-sampler-handoff.md.

Context:
We are moving the Melee frame-data pipeline from static per-frame generated capsules into a Rust 3D JObj/FigaTree sampler. The compact source manifest is canonical. It should preserve Melee XYZ floats and sample hit/hurt capsules on demand for the dev tool and runtime. Current 2D projection/flattening is only a view/runtime compatibility layer, not canonical storage.

Strict source-parity requirements:
- Do not guess mechanics.
- Do not patch around missing behavior.
- Reference the local Melee decomp before implementing each behavior.
- Treat Captain Falcon/Nair as the first validation target, not as a special-case architecture.
- Hitboxes should render red and hurtboxes yellow, but collision should be capsule-first in 3D before any current projection.

Start by verifying:
1. `git status --short --branch`
2. `git remote -v`
3. The source anchors listed in the handoff plan, especially:
   - `.research/doldecomp-melee/src/melee/ft/ftaction.c`
   - `.research/doldecomp-melee/src/melee/ft/ftcoll.c`
   - `.research/doldecomp-melee/src/melee/lb/lbcollision.c`
   - `.research/doldecomp-melee/src/melee/lb/types.h`
   - `.research/doldecomp-melee/src/melee/lb/lbanim.h`

Then execute the plan task-by-task with tests at each checkpoint. Keep the implementation agnostic to source character and action state. Do not exclude source states just because the Rust engine does not yet have a matching `MotionState`; those are parity gaps we need to surface, not decomp faults.
```
