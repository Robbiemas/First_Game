# Compact Melee Rig Frame Data Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace full-character per-frame frame-data materialization with a compact, Melee-shaped character/action manifest that can be expanded into dev-tool or runtime frame capsules on demand.

**Architecture:** The canonical import stores the same compact ingredients Melee uses: source action records, decoded subaction procedures, raw hurtbox init data from `ftData.x30`, skeleton/JObj references, and source file citations. Per-frame hitbox/hurtbox capsules remain derived views produced by the dev tool or runtime sampler, preserving Melee XYZ floats until the current 2D render/runtime flattens them.

**Tech Stack:** Rust `mole_cli`, JSON source artifacts under `resources/melee`, generated Rust runtime capsule tables in `crates/mole_runtime/src/generated`.

---

### Task 1: Lock The Compact Batch Contract

**Files:**
- Modify: `crates/mole_cli/tests/cli_contract.rs`

- [ ] **Step 1: Replace the all-states extraction assertion**

Assert that `mole frame-data extract --all-states --write` writes exactly one compact manifest under `resources/melee/frame_data/<target>/source_manifest.json`, includes all source actions including Rust parity gaps, preserves runtime bindings where they exist, and does not create one JSON artifact per state.

- [ ] **Step 2: Run the focused test and verify it fails**

Run: `cargo test -p mole_cli --test cli_contract frame_data_extract_all_states_creates_compact_source_manifest`

Expected: FAIL while the current implementation still writes per-state artifacts.

### Task 2: Build The Compact Manifest

**Files:**
- Modify: `crates/mole_cli/src/frame_data.rs`

- [ ] **Step 1: Add manifest path helpers**

Add `frame_data_manifest_path(root, target_character)` and source artifact helpers for `*_hurtbox_inits.json` and `*_costume_skeleton.json`.

- [ ] **Step 2: Replace the batch extraction loop**

Use `source_action_imports` to populate a single JSON object with:
- `schema_version`
- `artifact_kind: "source_character_frame_data_manifest"`
- `target_character`
- `source_character`
- `source_space: "melee_xyz"`
- `z_policy: "preserve_source_z_flatten_after_runtime_projection"`
- `rig_sources` pointing to `ftData.x30` hurtbox inits and skeleton/JObj data
- `actions` containing source action metadata and decoded procedures
- `rust_parity_gaps`

- [ ] **Step 3: Keep source floats as JSON numbers**

Copy existing parsed source JSON values directly. Do not stringify numeric vectors or round float fields.

- [ ] **Step 4: Write only the manifest when `--write` is passed**

Set `wrote_manifest`, `manifest_path`, and `manifest_bytes`; remove per-state write counters from the batch path or leave them as zero for backward report clarity.

### Task 3: Keep Single-State Expansion Available

**Files:**
- Modify: `crates/mole_cli/src/frame_data.rs`

- [ ] **Step 1: Leave `frame-data extract --state` as the targeted expanded view**

Do not remove the single-state artifact path yet. It remains useful for current dev-tool rendering and Nair verification.

- [ ] **Step 2: Mark sampled hurtbox/ECB data as derived**

Ensure batch extraction does not load `*_action_hurtbox_samples.json` or `*_action_ecb_samples.json`; those files are debug caches, not canonical full-character import data.

### Task 4: Runtime Export Boundary

**Files:**
- Modify: `crates/mole_cli/src/frame_data.rs`
- Test: `crates/mole_cli/tests/cli_contract.rs`

- [ ] **Step 1: Keep legacy expanded runtime export working**

Do not break `frame-data export-runtime --state`, because the current in-game Nair visualization depends on it.

- [ ] **Step 2: Make `export-runtime --all-states` refuse compact manifests until a sampler exists**

Return a clear source-shaped report explaining that all-state runtime export now requires implementing the Melee JObj/FigaTree sampler, instead of generating enormous static capsule tables.

### Task 5: Clean Generated Bloat Safely

**Files:**
- Modify: `resources/melee/frame_data/dolphin_mole/AttackAirN.json`
- Optional generated: `crates/mole_runtime/src/generated/frame_data_boxes.rs`

- [ ] **Step 1: Regenerate or shrink only artifacts created by this failed batch path**

Keep Nair playable and viewable, but stop carrying full-character precomputed artifacts as canonical output.

- [ ] **Step 2: Verify no untracked bulk state JSON files remain**

Run: `Get-ChildItem resources/melee/frame_data/dolphin_mole -Filter *.json`

Expected: only intentional artifacts remain.

### Task 6: Verification

**Files:**
- Test: `crates/mole_cli/tests/cli_contract.rs`
- Optional Test: `crates/mole_runtime/tests/runtime_contract.rs`

- [ ] **Step 1: Run formatting**

Run: `cargo fmt --all`

- [ ] **Step 2: Run focused CLI tests**

Run: `cargo test -p mole_cli --test cli_contract`

- [ ] **Step 3: Run runtime tests if generated runtime files changed**

Run: `cargo test -p mole_runtime`
