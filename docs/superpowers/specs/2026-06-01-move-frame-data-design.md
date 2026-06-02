# Move Frame Data Extraction and Keyframe Viewer Design

## Purpose

Replay-parity agents and human tool users need a fast way to extract attack/state frame data from the local Melee decomp and extracted resources, inspect the raw keyframes, and eventually turn that source data into editable Mole engine character data.

The first implementation should create the shared data shape and a read-only dev-tool view. It should not attempt a full editor or engine importer yet.

## Core Direction

Introduce a canonical `move_frame_data` JSON artifact as the bridge between:

- local decomp source semantics,
- extracted Melee DAT/action figatree data,
- the Mole CLI,
- the dev tool,
- and later engine import/export.

The artifact keeps Melee's source coordinates as 3D `{x, y, z}` values. Any 2D representation is recorded as projection metadata and applied only by a viewer or importer. The source data should not be destructively flattened because Mole may later support a 3D or deeper collision model.

For the current 2D Mole engine path, game-facing hitbox centers may be flattened onto the Z axis at the import/view boundary. Extracted artifacts should keep the source 3D offset alongside that flattened value, such as `source_center` for the decomp-executed Melee offset and `center` for the current 2D/game-facing coordinate. This preserves source Z for future 3D consumers while keeping the present Rust engine path playable and testable in 2D.

New extraction, schema, and import logic should prefer Rust so the dev tooling trends toward parity with the engine and avoids growing more Python tech debt. Python remains acceptable for the current Tk dev-tool shell and for narrow cases where it is clearly the fastest or most appropriate tool, but the reusable move-data pipeline should live in Rust wherever practical.

## Character Scope

Frame data is character-specific. The first extracted Captain Falcon attack data should be assigned to the current playable test character, `Dolphin Mole`, because Dolphin Mole is currently borrowing Captain Falcon data for parity work.

The dev tool must make this explicit:

- `Dolphin Mole` is the default selected character.
- A second empty test character is visibly available and switchable.
- Empty character/state selections should show an honest empty-state panel, not silently fall back to Dolphin Mole or global data.

This keeps the system character-scoped from day one and avoids building a global move database that becomes painful when the next character is added.

## First CLI Shape

Add a frame-data command family to Mole CLI:

```powershell
cargo run -p mole_cli -- frame-data extract --character dolphin_mole --source-character captain --state AttackAirN --json
cargo run -p mole_cli -- frame-data show --character dolphin_mole --state AttackAirN --format markdown
```

The command should default to read-only JSON output unless an explicit write flag is added later. A future `--write` option can persist artifacts under:

```text
resources/melee/frame_data/dolphin_mole/AttackAirN.json
```

For the first slice, the extractor may be conservative. If a value cannot be proven from decomp or extracted resources, it should emit `unknown` plus a `gaps` or `confidence` entry instead of guessing.

## Data Model

Each `move_frame_data` artifact should include:

- schema version,
- target character, such as `dolphin_mole`,
- source character, such as `captain`,
- motion state, such as `AttackAirN`,
- human label, such as `Neutral Air`,
- source citations,
- total frames,
- IASA/autocancel/landing-lag summary when proven,
- raw keyframes,
- hitbox definitions and active windows,
- hurt capsule definitions and active windows,
- ECB/body-volume definitions and active windows, kept separate from hurt capsules,
- projection metadata,
- extraction gaps,
- optional future overrides.

Sketch:

```json
{
  "schema_version": 1,
  "target_character": "dolphin_mole",
  "source_character": "captain",
  "state": "AttackAirN",
  "label": "Neutral Air",
  "projection": {
    "source_space": "melee_xyz",
    "default_view": "xy",
    "z_policy": "preserve_and_project"
  },
  "sources": [
    {
      "kind": "decomp",
      "path": "src/melee/ft/chara/ftCommon/ftCo_AttackAir.c",
      "line": 1,
      "purpose": "state semantics"
    },
    {
      "kind": "extracted_action",
      "path": "resources/melee/extracted/captain_falcon_action_animation_table.json",
      "purpose": "raw animation keyframes"
    }
  ],
  "summary": {
    "total_frames": 35,
    "iasa_frame": "unknown",
    "landing_lag_frames": "unknown",
    "active_hitbox_windows": []
  },
  "keyframes": [
    {
      "frame": 1,
      "interpolates_from_previous": false,
      "pose": [],
      "hitboxes": [],
      "hurtboxes": [],
      "body_volumes": []
    }
  ],
  "gaps": [
    {
      "field": "hitboxes",
      "reason": "not yet extracted from source tables"
    }
  ],
  "overrides": []
}
```

## Dev Tool Tab

Add a new tab to `tools/state_graph_viewer.py` named `Move Keyframes`.

The tab should prioritize raw keyframe inspection over a compact frame-data card:

- character selector with `Dolphin Mole` selected by default,
- second empty test character visible and switchable,
- state/move selector scoped to the selected character,
- timeline with per-frame cells and active-window coloring,
- selected-frame canvas,
- solid overlays for current hitboxes and hurtboxes,
- separate ECB/body-volume overlays,
- dashed or translucent overlays for interpolation from the previous keyframe,
- raw keyframe detail panel with IDs, bones, offsets, radii, damage/angle/knockback fields, source path/line, and confidence,
- empty-state panel when a character or state has no populated data.

The first version can draw simple projected circles/capsules on a Tk canvas. It does not need to be physically perfect. It must preserve and display the underlying `z` data in the detail panel even when rendering the default 2D projection.

`hurtboxes` means Melee `FighterHurtCapsule` / `HurtCapsule` data sourced from
`ftData.x30`, per-frame JObj pose, and action-script hurt-state commands.
`body_volumes` means ECB/body-volume samples sourced from `ftData.x44` and
`mpColl_LoadECB_JObj`-equivalent reduction. The two are related through the rig
but are not interchangeable and must not share provenance labels.

## Source Of Truth

Use decomp as the semantic source of truth for state behavior, callbacks, IASA/autocancel clues, and source citations.

Use extracted DAT/action figatree/ECB resources as the source for raw animation and geometry keyframes when those resources contain the needed data.

The extractor must keep these sources distinct in the artifact. Do not collapse all facts into a single uncited result.

## Future Edit And Export Path

Later work can add:

- manual dragging/resizing of selected hitboxes or hurtboxes,
- editable numeric fields,
- saved `overrides` beside extracted source data,
- CLI import/export into engine-side character tables,
- hot-reload or restart-based testing in the Mole runtime.

The first slice should prepare for this by separating extracted source data from overrides, but it should not implement mutation yet.

## Error Handling

The CLI and dev tool should fail softly:

- missing decomp root: explain expected path and allow `--decomp-root`,
- missing extracted resources: report the missing file,
- unsupported character/state: emit empty state with a clear message,
- unknown fields: preserve artifact validity and record gaps.

## Testing

Add focused contract tests for:

- CLI help exposes `frame-data extract` and `frame-data show`,
- extraction for a fixture state emits target/source character metadata,
- output preserves 3D coordinates and projection metadata,
- unknown fields are represented as gaps rather than guessed values,
- dev tool loaders handle populated Dolphin Mole data and an empty second test character,
- generated markdown summaries include source citations.

## First Implementation Slice

1. Add the `move_frame_data` schema conventions and fixture artifact for `dolphin_mole` / `AttackAirN`.
2. Add CLI read-only `frame-data extract` and `frame-data show`.
3. Add the `Move Keyframes` dev-tool tab with Dolphin Mole selected by default and an empty second test character.
4. Render the timeline, selected-frame projected overlays, interpolation ghosts, and raw detail panel from the artifact.
5. Add tests and README/help examples.

This is enough for agents to request “Captain Falcon Nair frame data,” receive a structured artifact assigned to Dolphin Mole, and inspect it in the dev tool without committing to final engine import semantics.
