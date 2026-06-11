# Parity Ledger And Character Editor Foundation Design

Date: 2026-05-30

## Purpose

Turn the existing side-by-side Melee/Mole state graph viewer into the central
movement parity ledger. The immediate job is to make grounded movement and
moonwalk debugging less guessy: every state, transition, and physics value
should point to its Melee decomp source, Rust owner, tests, and known gaps.

The long-term job is larger. This same tooling should become the foundation for
a character-development editor: first for the Captain Falcon-modeled Dolphin
Mole test character, later for new characters with intentionally edited values.
The editor direction must not change gameplay authority. Rust data and
deterministic simulation remain authoritative; the tool inspects, documents, and
eventually edits explicit data seams.

This must also stay engine-agnostic. The Rust backend is the durable simulation
and data authority whether the first runtime stays SDL, grows into a custom Rust
renderer, or is later embedded behind Unity, Godot, or another 2D/3D frontend.
Rendering, editor UI, and host-engine integration should be replaceable shells
around the same deterministic Rust state, inputs, value tables, and replayable
simulation contracts.

## Goals

- Preserve the existing dual graph workflow that opens with the game.
- Make graph nodes and edges definitive parity records instead of loose visual
notes.
- Add global/common value sheets from extracted `PlCo.dat`.
- Add character value sheets from extracted Captain Falcon `PlCa.dat`.
- Show source provenance for every value: Rust field, decomp/source name,
  offset, raw value, converted Rust value, unit/kind, and status.
- Keep the first implementation checkpoint focused on bottom-up grounded
  locomotion: `Wait`, `WalkSlow`, `WalkMiddle`, `WalkFast`, `Turn`, `Dash`,
  `Run`, `RunBrake`, and `TurnRun`.
- Make future editor behavior possible without making the viewer an
  authoritative runtime dependency.

## Non-Goals

- Do not tune moonwalk by feel inside this setup pass.
- Do not add a `Moonwalk` state or any moonwalk-specific gameplay branch.
- Do not move gameplay values into Python, Tkinter, JSON graph notes, or the SDL
  runtime.
- Do not make Pygame authoritative again.
- Do not build a full character editor UI in this first checkpoint.
- Do not infer missing Melee behavior from intuition. Missing source evidence
  stays marked as a gap.
- Do not couple the parity data model to SDL, Tkinter, Pygame, or any future
  Unity/Godot frontend.

## Data Model

The graph data should evolve from simple display metadata into a small parity
ledger schema.

Each node should be able to carry:

- `id` and `label`
- parity `status`
- concise `notes`
- `source_refs`: local decomp files, functions, structs, or offsets
- `rust_refs`: Rust files/functions/tests that own or verify the behavior
- `value_refs`: global or character values used by the state
- `known_gaps`: explicit missing source data, implementation gaps, or test gaps

Each edge should carry the same reference fields, plus:

- `input`: what input or condition causes the transition
- `frames`: source timing or current verified timing
- `physics`: velocity, acceleration, friction, timer, or state-local fields used
  by the transition

The existing graph files remain the human-authored review surface:

- `docs/state_graphs/melee_reference_graph.json`
- `docs/state_graphs/mole_current_graph.json`

Generated value sheets should live beside them and be safe to refresh from the
existing extraction outputs:

- `docs/state_graphs/value_sheets/global_common_values.json`
- `docs/state_graphs/value_sheets/captain_falcon_values.json`

These generated sheets can be displayed by the viewer now and can later become
the editable source for a proper character editor once there is an explicit
save/apply pipeline.

## Value Sheets

The global/common sheet comes from `resources/melee/extracted/plco_common_data.json`.
It should group fields into practical categories:

- input thresholds and timers
- grounded locomotion
- jump and aerial control
- shield, roll, spotdodge, and platform drop
- escape-air and special-fall
- animation and shared multipliers

The character sheet comes from
`resources/melee/extracted/captain_falcon_profile.json`. It should group fields
into:

- walk and run
- dash and turn
- jump and aerial movement
- gravity and fall
- landing and action frame data
- visual/editor metadata

The displayed values must keep both raw and converted representations. For
example, a decomp float of `2.0` for Falcon's `dash_initial_velocity` should be
shown next to the Rust milli-unit value `2000`. This is important because
future character editing will need to distinguish source values from our fixed
point runtime representation.

## Viewer Behavior

The old Tkinter viewer was acceptable for the first checkpoint only. The
current durable human-facing path is the Rust dev tool. The
design priority is clarity, not UI polish.

The viewer should add:

- a tab or mode for the dual graph
- a tab or mode for global/common values
- a tab or mode for character values
- richer details when a node or edge is clicked
- summary counts for aligned, partial, mismatch, missing, and intentional items
- stable validation so malformed ledger fields fail tests before the viewer
  opens

Clicking `Dash`, for example, should surface the source files/functions,
`PlCo.dat` fields, Falcon profile fields, Rust functions, and tests that define
the current behavior. If moonwalk still fails by feel, the graph should tell us
which Dash-related facts are proven and which are still partial.

The viewer must consume data through plain files and stable Rust-owned schemas.
That keeps the current tool disposable: a future Godot, Unity, web, or custom
Rust editor can render the same parity ledger and value sheets without changing
gameplay code.

## Future Character Editor Direction

The future editor should be data-first:

1. Load a reference profile, such as Captain Falcon.
2. Show every global and character-owned gameplay value with source provenance.
3. Allow creating a derived character profile with explicit overrides.
4. Save those overrides into a Rust-readable data file or generated Rust module.
5. Run contract tests and deterministic replay checks before the profile is
   considered usable.

This means the first checkpoint should avoid hard-coding UI assumptions that
only work for Captain Falcon. Tables should be keyed by field identity and
category, not by one-off labels. The current Dolphin Mole profile is the first
editable character candidate, but the schema should allow more characters later.

The eventual editor should treat visuals as replaceable consumers of Rust core
state. A 3D frontend may map the bottom ECB vertex, facing, motion state,
physics values, and animation cues onto a rig or scene graph, but it should not
redefine movement, collision, or rollback state. The same profile and common
value sheets should be able to drive a 2D sprite runtime, a 3D prototype, or a
host-engine plugin.

## Bottom-Up Parity Workflow

Work should proceed from the lowest grounded movement layer upward:

1. `Wait`
2. `WalkSlow`, `WalkMiddle`, `WalkFast`
3. `Turn`
4. `Dash`
5. `Run`
6. `RunBrake`
7. `TurnRun`
8. `KneeBend` and jump carry
9. airborne drift, air dodge, `FallSpecial`, and landing
10. shield and defensive movement
11. attacks, hitboxes, hurtboxes, knockback, hitlag, and combat systems

For each item, the parity ledger should answer:

- What does Melee call this state or transition?
- Which decomp file/function proves it?
- Which Rust state/function owns it?
- Which extracted values drive it?
- Which tests prove it?
- What remains partial or missing?

## Testing

Tests should cover the tooling itself before it becomes trusted:

- graph JSON validation accepts the enriched ledger fields
- generated value sheets contain required field identities and provenance
- the global sheet includes key `PlCo.dat` movement fields such as dash
  threshold, tap window, Dash IASA windows, run threshold, run taper, and
  friction multiplier
- the Falcon sheet includes walk, dash, run, traction, jump, gravity, fall,
  air-drift, and landing fields
- `tools/state_graph_viewer.py --check` prints graph and sheet summaries
- existing runtime launcher still opens the state graph viewer with the game

Gameplay tests remain in Rust and remain authoritative. The parity ledger can
point to tests and reveal gaps, but it does not replace deterministic core
tests.

## First Implementation Checkpoint

The first checkpoint is a documentation/tooling checkpoint:

1. add a value-sheet generator that reads the existing extracted Melee JSON
2. generate global/common and Captain Falcon value sheets
3. enrich the grounded locomotion graph nodes and edges with source/Rust/test
   references
4. update the viewer's validation and details panel to display ledger fields
   and value-sheet summaries
5. add focused Python tests for the graph/value-sheet tooling
6. run the existing Rust verification gate afterward to make sure no gameplay
   authority moved
