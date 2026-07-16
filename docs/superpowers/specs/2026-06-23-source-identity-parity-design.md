# Source Identity Parity Design

## Goal

Make extracted Melee table values first-class source identities in the Rust engine, CLI, runtime diagnostics, and devtool surfaces so that decomp values are preserved instead of being silently translated into Rust-only IDs.

## Problem

The current pipeline has an identity ambiguity. In the frame-142 EscapeAir investigation, the decomp and extraction show two valid Melee identities for the same gameplay concept:

- `ftCo_MS_EscapeAir` is common motion state `236`.
- Captain Falcon's action animation table row for `PlyCaptain5K_Share_ACTION_EscapeAir_figatree` is `44`.

Both values are true. The problem is that artifacts and code often call a table row `action_state_id`, while Rust also has runtime `MotionState` variants and Melee common motion-state IDs. That makes it look like the engine is translating one source value into another, when the real structure is multiple source tables connected by explicit bindings.

The design objective is not to collapse Melee's tables into one ID space. The objective is to preserve each source ID space, name it correctly, and require every cross-space relationship to be represented as typed data.

## Source Identity Spaces

The engine should distinguish these identities everywhere they cross a boundary:

| Identity | Meaning | Example |
| --- | --- | --- |
| `MeleeMotionStateId` | Decomp motion-state enum value such as `ftCo_MS_*` or character special motion states | `236` for `ftCo_MS_EscapeAir` |
| `SourceActionTableIndex` | Row index in a character action animation table, currently `ftData.xC` / `Fighter_WaitAnimData` | `44` for Falcon `EscapeAir` |
| `SourceActionKey` | Stable symbolic source key used by CLI/devtool and generated bindings | `"EscapeAir"` |
| `SourceAnimationRef` | The exact animation resource referenced by the source action table row | `PlyCaptain5K_Share_ACTION_EscapeAir_figatree`, archive offset, size |
| `RuntimeMotionVariant` | Rust control-flow enum variant used to route implemented behavior | `MotionState::EscapeAir` |
| `RuntimeActionBinding` | The explicit binding between source identities and the Rust runtime variant | `motion_state_id=236`, `action_table_index=44`, `source_action_key="EscapeAir"`, `runtime_variant=EscapeAir` |

The Rust engine may keep a `MotionState` enum for ergonomic control flow, but it must not act as the source identity. It is a runtime variant bound to source identities.

## Chosen Approach

Use typed source IDs and versioned binding records.

This is better than renumbering Rust variants to match one source table because Melee has more than one relevant source table. Renumbering would only move the ambiguity around. A single source concept can have a common motion-state ID, a character animation table index, a subaction script offset, a FigaTree reference, callback table entries, and character-specific state IDs.

The chosen model is:

```text
decomp table values
  -> typed extraction identities
  -> explicit source identity binding
  -> compact generated runtime binding
  -> Rust runtime variant for control flow
```

For frame-142 EscapeAir, the binding should make this unambiguous:

```text
MeleeMotionStateId(236)        ftCo_MS_EscapeAir
SourceActionTableIndex(44)    Falcon action animation table row
SourceActionKey("EscapeAir")  stable source symbol
RuntimeMotionVariant          MotionState::EscapeAir
```

## Artifact Shape

Extraction and generated metadata should stop using unqualified `action_state_id` for action animation table rows. During migration, old fields can remain as deprecated aliases, but new consumers should use explicit fields.

Recommended source-side shape:

```json
{
  "schema_version": 2,
  "source_character": "captain",
  "source_action_key": "EscapeAir",
  "melee_motion_state": {
    "id": 236,
    "symbol": "ftCo_MS_EscapeAir",
    "source_file": "src/melee/ft/chara/ftCommon/ftCo_EscapeAir.c"
  },
  "action_table": {
    "kind": "Fighter_WaitAnimData",
    "index": 44,
    "source_file": "resources/melee/raw/PlCa.dat",
    "table_offset": 0
  },
  "animation": {
    "figatree_root": "PlyCaptain5K_Share_ACTION_EscapeAir_figatree",
    "archive_file": "resources/melee/raw/PlCaAJ.dat",
    "archive_offset": 0,
    "archive_size": 0
  },
  "runtime_binding": {
    "runtime_motion_variant": "EscapeAir",
    "binding_kind": "common_motion_state_to_character_action"
  }
}
```

Runtime-facing generated data should keep this compact. It should store numeric IDs and static keys, not provenance strings:

```rust
pub struct RuntimeActionIdentity {
    pub melee_motion_state_id: Option<MeleeMotionStateId>,
    pub source_action_table_index: SourceActionTableIndex,
    pub source_action_key: SourceActionKey,
    pub runtime_motion_variant: Option<MotionState>,
}
```

If `SourceActionKey` remains a static string wrapper temporarily, it is acceptable for CLI/devtool compatibility. Long-term rollback snapshots should prefer numeric IDs or table indices, not string/provenance data.

## Runtime Rules

- Normal play must consume compact generated Rust or binary runtime data only.
- Runtime must not parse the source JSON identity artifacts during gameplay.
- Runtime state may use Rust `MotionState` for callback routing, but replay diagnostics must be able to print the source IDs that justify that route.
- Rollback snapshots should store compact authoritative IDs and state, not source file paths, offsets, JSON objects, or editor provenance.
- Any runtime behavior that depends on an action's source data should lookup by typed source identity, not by a raw integer with ambiguous meaning.

## CLI And Devtool Rules

The CLI and devtool should show identity spaces side by side. For a selected action, they should report:

- Melee motion-state ID and symbol.
- Character action table index.
- Source action key.
- FigaTree/root animation reference.
- Runtime motion variant, if implemented.
- Known gaps if the source action exists but runtime behavior is not yet implemented.

Import plans such as "Marth down tilt into dev character 2" should carry the source action table identity and the target runtime binding separately. The extracted source artifact must not be mutated.

## Migration Strategy

1. Add typed IDs and identity reports without changing gameplay behavior.
2. Add compatibility readers for old `action_state_id` fields while writing new `action_table_index` fields.
3. Update generated runtime bindings to carry both `melee_motion_state_id` and `source_action_table_index`.
4. Update diagnostics so divergence output names the exact identity space involved.
5. Rename ambiguous code and artifact fields gradually after tests prove equivalent behavior.

## Acceptance Criteria

- Falcon `EscapeAir` can be reported as `melee_motion_state_id=236` and `source_action_table_index=44` in the same diagnostic without conflict.
- No generated runtime artifact needs to translate `44` into a Rust-only number to find EscapeAir source data.
- Existing replay tests continue to pass after identity migration.
- `runtime-data size-report` remains within the compact runtime contract.
- Debug/sample JSON remains outside normal runtime consumption.

## Non-Goals

- Do not renumber all Rust enum variants as a quick fix.
- Do not collapse Melee common motion states and character action table rows into one table.
- Do not move rich provenance into rollback snapshots.
- Do not make runtime parse JSON or decomp files during normal play.
- Do not resolve Slippi quirks by changing source identities.
