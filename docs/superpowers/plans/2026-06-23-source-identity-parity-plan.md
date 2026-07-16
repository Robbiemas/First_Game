# Source Identity Parity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Preserve Melee table values as typed source identities across extraction, generated runtime data, diagnostics, and devtool/CLI surfaces.

**Architecture:** Add explicit identity types for Melee motion-state IDs and character action table indices, then bind them through compact generated runtime records. Keep Rust `MotionState` as a control-flow variant only, not as the source identity. Runtime remains compact and preload-only; rich provenance stays in CLI/devtool artifacts.

**Tech Stack:** Rust workspace crates `mole_core`, `mole_cli`, `mole_runtime`; Python extractor `tools/extract_melee_resources.py`; JSON extraction artifacts under `resources/melee/extracted`; existing Mole CLI test runner.

---

## File Structure

- Modify `crates/mole_core/src/state.rs`: add typed identity newtypes and runtime binding structs, then migrate ambiguous binding helpers.
- Modify `crates/mole_core/src/lib.rs`: re-export new identity types.
- Modify `tools/extract_melee_resources.py`: write `action_table_index` in extracted action animation and ECB sample artifacts while preserving the old field as a deprecated alias during migration.
- Modify `tests/test_extract_melee_resources.py`: assert extraction preserves both typed action table identity and compatibility alias.
- Modify `crates/mole_cli/src/frame_data.rs`: surface the typed identities in frame-data catalog/sample/export outputs.
- Modify `crates/mole_cli/src/runtime_data.rs`: add or extend an identity report under runtime-data tooling.
- Modify `crates/mole_cli/tests/cli_contract.rs`: assert CLI JSON and generated Rust include explicit identity spaces.
- Modify `crates/mole_runtime/src/slippi_diagnostic.rs`: print source identities in replay divergence diagnostics.
- Modify `crates/mole_runtime/tests/runtime_contract.rs`: assert frame-142 EscapeAir diagnostics include motion ID `236` and action table index `44`.
- Modify docs only after code is green: update `docs/architecture/runtime-baked-data-performance-contract.md` and `docs/architecture/rust-devtool-lossless-middleware.md` with the identity contract.

## Task 1: Add Core Source Identity Types

**Files:**
- Modify: `crates/mole_core/src/state.rs`
- Modify: `crates/mole_core/src/lib.rs`

- [ ] **Step 1: Write the failing core identity test**

Add a unit test near the existing source binding tests in `crates/mole_core/src/state.rs`:

```rust
#[test]
fn escape_air_binding_preserves_motion_state_and_action_table_id_spaces() {
    let binding = source_binding_for_motion_state(MotionState::EscapeAir)
        .expect("EscapeAir must have a source binding");

    assert_eq!(binding.melee_motion_state_id, Some(MeleeMotionStateId::new(236)));
    assert_eq!(binding.source_action_table_index, SourceActionTableIndex::new(44));
    assert_eq!(binding.source_action_key.as_str(), "EscapeAir");
    assert_eq!(binding.runtime_motion_state, Some(MotionState::EscapeAir));
}
```

- [ ] **Step 2: Run the test and verify it fails**

Run:

```powershell
cargo run -q -p mole_cli -- tests run -p mole_core escape_air_binding_preserves_motion_state_and_action_table_id_spaces
```

Expected: fail because `MeleeMotionStateId`, `SourceActionTableIndex`, or the new binding fields do not exist yet.

- [ ] **Step 3: Add minimal identity types**

In `crates/mole_core/src/state.rs`, add compact newtypes near `MeleeActionStateId` and `SourceActionKey`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MeleeMotionStateId(u16);

impl MeleeMotionStateId {
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u16 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceActionTableIndex(u16);

impl SourceActionTableIndex {
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u16 {
        self.0
    }
}
```

Update `MotionStateSourceBinding` to carry both identities:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MotionStateSourceBinding {
    pub runtime_motion_state: Option<MotionState>,
    pub melee_motion_state_id: Option<MeleeMotionStateId>,
    pub source_action_table_index: SourceActionTableIndex,
    pub source_action_key: SourceActionKey,
}
```

Update its constructor to accept a runtime state, source action table index, and key:

```rust
impl MotionStateSourceBinding {
    pub const fn new(
        runtime_motion_state: MotionState,
        source_action_table_index: u16,
        source_action_key: &'static str,
    ) -> Self {
        Self {
            runtime_motion_state: Some(runtime_motion_state),
            melee_motion_state_id: melee_action_state_id_for_motion_state(runtime_motion_state),
            source_action_table_index: SourceActionTableIndex::new(source_action_table_index),
            source_action_key: SourceActionKey::new(source_action_key),
        }
    }
}
```

Keep existing call sites compiling by updating field names only. Do not change gameplay behavior.

- [ ] **Step 4: Re-export the types**

In `crates/mole_core/src/lib.rs`, add the new types to the existing `pub use state::{ ... }` list:

```rust
MeleeMotionStateId, SourceActionTableIndex,
```

- [ ] **Step 5: Run the core test and existing binding tests**

Run:

```powershell
cargo run -q -p mole_cli -- tests run -p mole_core escape_air_binding_preserves_motion_state_and_action_table_id_spaces
```

Expected: pass.

## Task 2: Rename Extraction Output Semantics Without Breaking Readers

**Files:**
- Modify: `tools/extract_melee_resources.py`
- Modify: `tests/test_extract_melee_resources.py`

- [ ] **Step 1: Write the failing extraction test**

In `tests/test_extract_melee_resources.py`, extend the existing `test_extract_captain_action_animation_table_links_plca_records_to_plcaaj_chunks` test with these assertions:

```python
assert extracted["actions"][0]["identity_kind"] == "fighter_wait_anim_data_index"
assert extracted["actions"][0]["action_table_index"] == 0
assert extracted["actions"][0]["action_state_id"] == 0
assert extracted["actions"][1]["identity_kind"] == "fighter_wait_anim_data_index"
assert extracted["actions"][1]["action_table_index"] == 1
assert extracted["actions"][1]["action_state_id"] == 1
```

Also extend `test_extract_captain_escape_air_script_skip_decay_gate_from_real_resources` with:

```python
assert escape_air["identity_kind"] == "fighter_wait_anim_data_index"
assert escape_air["action_table_index"] == 44
assert escape_air["action_state_id"] == 44
```

- [ ] **Step 2: Run the test and verify it fails**

Run:

```powershell
pytest tests/test_extract_melee_resources.py -k action_animation_table_uses_action_table_index_identity -q
```

Expected: fail because `action_table_index` and `identity_kind` are not emitted yet.

- [ ] **Step 3: Write the minimal extractor change**

In `extract_character_action_animation_table`, change each emitted action object to include explicit identity fields:

```python
"identity_kind": "fighter_wait_anim_data_index",
"action_table_index": action_state_id,
"action_state_id": action_state_id,
```

Do the same in `extract_captain_action_ecb_samples` for sampled action records:

```python
"identity_kind": "fighter_wait_anim_data_index",
"action_table_index": action_state_id,
"action_state_id": action_state_id,
```

Leave `action_state_id` as a deprecated compatibility alias for this migration slice.

- [ ] **Step 4: Run the extraction test**

Run:

```powershell
pytest tests/test_extract_melee_resources.py -k action_animation_table_uses_action_table_index_identity -q
```

Expected: pass.

## Task 3: Generate Runtime Bindings With Both Identity Spaces

**Files:**
- Modify: `crates/mole_core/src/state.rs`
- Modify: `crates/mole_cli/src/frame_data.rs`
- Modify: `crates/mole_cli/tests/cli_contract.rs`

- [ ] **Step 1: Write the failing CLI generation test**

In `crates/mole_cli/tests/cli_contract.rs`, add an assertion to
`frame_data_export_runtime_all_states_compact_manifest_writes_compact_source_export`, the runtime
export test that already checks generated action bindings:

```rust
assert!(
    generated.contains(
        "RuntimeActionBinding { melee_motion_state_id: Some(MeleeMotionStateId::new(236)), source_action_table_index: SourceActionTableIndex::new(44), source_action_key: \"EscapeAir\", motion_state: Some(MotionState::EscapeAir) }"
    ),
    "EscapeAir binding must preserve common motion-state id 236 and Falcon action table index 44"
);
```

- [ ] **Step 2: Run the test and verify it fails**

Run:

```powershell
cargo run -q -p mole_cli -- tests run -p mole_cli runtime_export_includes_source_only_action_bindings
```

Expected: fail because this new focused test does not exist yet. Add it as a wrapper around the
same compact export fixture, or run the existing compact export test directly while developing this
slice:

```powershell
cargo run -q -p mole_cli -- tests run -p mole_cli frame_data_export_runtime_all_states_compact_manifest_writes_compact_source_export
```

After the focused test is added, it should fail because generated binding text still emits the older
ambiguous shape.

- [ ] **Step 3: Update runtime binding struct shape**

Where `RuntimeActionBinding` is generated or defined, change the fields to:

```rust
pub struct RuntimeActionBinding {
    pub melee_motion_state_id: Option<MeleeMotionStateId>,
    pub source_action_table_index: SourceActionTableIndex,
    pub source_action_key: &'static str,
    pub motion_state: Option<MotionState>,
}
```

For source-only actions without a runtime motion state, set `melee_motion_state_id` to the source motion/action ID only if the decomp proves it is a motion-state ID. Otherwise leave it `None` and rely on `source_action_table_index`.

- [ ] **Step 4: Update generated output construction**

In `crates/mole_cli/src/frame_data.rs`, the generated EscapeAir binding must emit exactly this shape:

```rust
RuntimeActionBinding {
    melee_motion_state_id: Some(MeleeMotionStateId::new(236)),
    source_action_table_index: SourceActionTableIndex::new(44),
    source_action_key: "EscapeAir",
    motion_state: Some(MotionState::EscapeAir),
}
```

The generated Attack12 source-only binding must emit exactly this shape:

```rust
RuntimeActionBinding {
    melee_motion_state_id: None,
    source_action_table_index: SourceActionTableIndex::new(45),
    source_action_key: "Attack12",
    motion_state: None,
}
```

- [ ] **Step 5: Run the CLI generation test**

Run:

```powershell
cargo run -q -p mole_cli -- tests run -p mole_cli runtime_export_includes_source_only_action_bindings
```

Expected: pass.

## Task 4: Add A CLI Identity Report

**Files:**
- Modify: `crates/mole_cli/src/runtime_data.rs`
- Modify: `crates/mole_cli/src/lib.rs`
- Modify: `crates/mole_cli/tests/cli_contract.rs`

- [ ] **Step 1: Write the failing identity report test**

In `crates/mole_cli/tests/cli_contract.rs`, add:

```rust
#[test]
fn runtime_data_identity_report_separates_escape_air_ids() {
    let output = run_cli(&[
        "runtime-data".to_string(),
        "identity-report".to_string(),
        "--character".to_string(),
        "captain".to_string(),
        "--action".to_string(),
        "EscapeAir".to_string(),
        "--json".to_string(),
    ])
    .unwrap();
    let output: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(output["source_action_key"], "EscapeAir");
    assert_eq!(output["melee_motion_state"]["id"], 236);
    assert_eq!(output["action_table"]["index"], 44);
    assert_eq!(output["runtime_binding"]["motion_state"], "EscapeAir");
}
```

- [ ] **Step 2: Run the failing CLI test**

Run:

```powershell
cargo run -q -p mole_cli -- tests run -p mole_cli runtime_data_identity_report_separates_escape_air_ids
```

Expected: fail because the command does not exist.

- [ ] **Step 3: Implement the report using existing generated bindings**

Add a `runtime-data identity-report` subcommand that reads compact generated binding metadata already linked into Rust. The command must not parse gameplay JSON during normal runtime. Its JSON output for EscapeAir should include:

```json
{
  "source_character": "captain",
  "source_action_key": "EscapeAir",
  "melee_motion_state": {
    "id": 236,
    "symbol": "ftCo_MS_EscapeAir"
  },
  "action_table": {
    "kind": "Fighter_WaitAnimData",
    "index": 44
  },
  "runtime_binding": {
    "motion_state": "EscapeAir"
  }
}
```

- [ ] **Step 4: Run the CLI identity test**

Run:

```powershell
cargo run -q -p mole_cli -- tests run -p mole_cli runtime_data_identity_report_separates_escape_air_ids
```

Expected: pass.

## Task 5: Carry Source Identities Into Replay Diagnostics

**Files:**
- Modify: `crates/mole_runtime/src/slippi_diagnostic.rs`
- Modify: `crates/mole_runtime/tests/runtime_contract.rs`

- [ ] **Step 1: Write the failing runtime diagnostic test**

In `crates/mole_runtime/tests/runtime_contract.rs`, update `slippi_match_start_frame142_p2_escape_air_soft_platform_commit_is_mixed_phase_witness` by adding assertions after the existing `player.source_floor_for_diagnostic()` assertion. The implementation should expose source identity through a compact `PlayerState::source_action_identity_for_diagnostic()` helper.

```rust
let source_identity = player
    .source_action_identity_for_diagnostic()
    .expect("EscapeAir must expose source identity");

assert_eq!(source_identity.melee_motion_state_id.map(|id| id.get()), Some(236));
assert_eq!(source_identity.source_action_table_index.get(), 44);
assert_eq!(source_identity.source_action_key.as_str(), "EscapeAir");
```

- [ ] **Step 2: Run the failing runtime test**

Run:

```powershell
cargo run -q -p mole_cli -- tests run -p mole_runtime frame142_escape_air_diagnostic_names_both_source_id_spaces
```

Expected: fail because diagnostics do not include `source_identity`.

- [ ] **Step 3: Add compact diagnostic projection**

In `slippi_diagnostic.rs`, when projecting Rust player state into a diagnostic row, attach a small source identity object:

```rust
"source_identity": {
    "melee_motion_state_id": binding.melee_motion_state_id.map(|id| id.get()),
    "source_action_table_index": binding.source_action_table_index.get(),
    "source_action_key": binding.source_action_key.as_str(),
}
```

This is diagnostic output only. Do not add provenance strings to rollback snapshots.

- [ ] **Step 4: Run the runtime diagnostic test**

Run:

```powershell
cargo run -q -p mole_cli -- tests run -p mole_runtime frame142_escape_air_diagnostic_names_both_source_id_spaces
```

Expected: pass.

## Task 6: Add Migration Guardrails For Ambiguous Raw IDs

**Files:**
- Modify: `crates/mole_core/tests/core_contract.rs`
- Modify: `crates/mole_cli/tests/cli_contract.rs`

- [ ] **Step 1: Write a failing guardrail test for ambiguous generated fields**

Add a test that reads generated binding text and rejects new ambiguous fields:

```rust
#[test]
fn generated_runtime_bindings_do_not_emit_ambiguous_action_state_id_field() {
    let generated = generate_runtime_frame_data_for_test();

    assert!(
        !generated.contains("action_state_id:"),
        "new generated runtime bindings must use melee_motion_state_id or source_action_table_index"
    );
    assert!(generated.contains("source_action_table_index:"));
}
```

Add the guardrail assertion to the existing
`frame_data_export_runtime_all_states_compact_manifest_writes_compact_source_export` test, which
already has the generated Rust text in a local `generated` variable.

- [ ] **Step 2: Run the guardrail test and verify it fails**

Run:

```powershell
cargo run -q -p mole_cli -- tests run -p mole_cli generated_runtime_bindings_do_not_emit_ambiguous_action_state_id_field
```

Expected: fail until generated binding code is fully migrated.

- [ ] **Step 3: Migrate remaining generated binding field names**

Replace generated runtime uses of `action_state_id:` with either:

```rust
melee_motion_state_id:
```

or:

```rust
source_action_table_index:
```

based on the source table being represented. If a raw ID cannot be classified, stop and add it to the parity gap ledger instead of guessing.

- [ ] **Step 4: Run the guardrail test**

Run:

```powershell
cargo run -q -p mole_cli -- tests run -p mole_cli generated_runtime_bindings_do_not_emit_ambiguous_action_state_id_field
```

Expected: pass.

## Task 7: Update Architecture Docs

**Files:**
- Modify: `docs/architecture/rust-devtool-lossless-middleware.md`
- Modify: `docs/architecture/runtime-baked-data-performance-contract.md`

- [ ] **Step 1: Add the source identity contract to middleware docs**

Add this rule under the lossless artifact section:

```markdown
Source identity fields must be typed by source table. A Melee common motion-state ID, a character action animation table index, a FigaTree archive reference, and a Rust runtime variant are separate identities connected by explicit binding records. Do not write or consume unqualified `action_state_id` fields in new artifacts.
```

- [ ] **Step 2: Add the compact runtime rule**

Add this rule under runtime data rules:

```markdown
Runtime bindings may store compact source IDs such as `MeleeMotionStateId` and `SourceActionTableIndex`. They must not store rich source provenance, file paths, or JSON objects in hot gameplay state or rollback snapshots.
```

- [ ] **Step 3: Run docs-neutral tests**

Run:

```powershell
cargo run -q -p mole_cli -- tests run -p mole_cli runtime_data_identity_report_separates_escape_air_ids
cargo run -q -p mole_cli -- tests run -p mole_runtime frame142_escape_air_diagnostic_names_both_source_id_spaces
```

Expected: pass.

## Task 8: Final Verification

**Files:**
- No new files.

- [ ] **Step 1: Run extraction tests**

Run:

```powershell
pytest tests/test_extract_melee_resources.py -q
```

Expected: pass.

- [ ] **Step 2: Run focused Rust tests**

Run:

```powershell
cargo run -q -p mole_cli -- tests run -p mole_core escape_air_binding_preserves_motion_state_and_action_table_id_spaces escape_air_mp_coll_load_ecb_jobj_samples_current_pose_for_soft_floor_parity
cargo run -q -p mole_cli -- tests run -p mole_cli runtime_data_identity_report_separates_escape_air_ids generated_runtime_bindings_do_not_emit_ambiguous_action_state_id_field
cargo run -q -p mole_cli -- tests run -p mole_runtime frame142_escape_air_diagnostic_names_both_source_id_spaces slippi_match_start_frame142_p2_escape_air_soft_platform_commit_is_mixed_phase_witness
```

Expected: pass.

- [ ] **Step 3: Run runtime size report**

Run:

```powershell
cargo run -q -p mole_cli -- runtime-data size-report --json
```

Expected: existing known warnings may remain, but no new runtime JSON/debug artifact dependency appears.

- [ ] **Step 4: Run the frame-142 trace**

Run:

```powershell
cargo run -q -p mole_cli -- replay trace --inputs debug\slippi\Game_20260530T214929.full.inputs.json --mode match-start --player 2 --start 138 --end 143 --frames 320 --json
```

Expected: the report explains Rust's EscapeAir identity as common motion-state `236` bound to Falcon source action table index `44`.

## Self-Review Notes

- The plan does not renumber Rust enum variants.
- The plan does not collapse Melee source tables.
- The plan preserves runtime compactness by storing typed numeric IDs in generated data.
- The plan leaves old extraction aliases in place during migration so existing tools can be moved safely.
- The plan requires tests before production changes for every behavior slice.
