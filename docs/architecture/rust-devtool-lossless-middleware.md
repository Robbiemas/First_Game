# Rust Dev Tool Lossless Middleware Contract

Date: 2026-06-11

This document is the entry point for work that touches the Melee decomp extraction pipeline, Mole CLI, Rust dev tool GUI, or Rust engine import path.

## North Star

The project is moving away from Python and Pygame. Python files in this repository are historical reference or temporary legacy extraction helpers only. New durable tooling and UI work belongs in Rust.

The Rust dev tool is not just a viewer. It is the middleware layer between the Melee decomp reference and the Rust engine:

```text
Melee decomp/reference data
  -> Mole CLI extraction/generation
  -> Rust-owned middleware artifacts
  -> Rust dev tool GUI inspection/editing
  -> Mole CLI validation/import/export
  -> Rust engine/runtime consumption
```

The middleware must be lossless. If source data is extracted from the decomp, edited in the GUI, validated through the CLI, and imported into the engine, the artifact must preserve enough data to explain exactly where every runtime value came from and what was changed.

## Four-Way Parity Rule

Every parity feature must satisfy all four surfaces:

- **Decomp/reference:** source values, callbacks, tables, offsets, and provenance remain inspectable.
- **Rust engine:** runtime consumes the artifact without hidden conversion or semantic drift.
- **Mole CLI:** agents can extract, inspect, diff, edit/apply, validate, regenerate, and import headlessly.
- **Rust dev tool GUI:** humans can see and edit the same data visually.

The CLI and GUI are dual surfaces. Anything added to one must be represented on the other. A GUI-only edit path is incomplete. A CLI-only inspection path is incomplete.

Treat the Mole CLI as the primary truth and automation contract. Agents use the CLI directly; the Rust dev tool GUI should build on the same Rust command/view-model functions and artifact schemas so humans and agents are always operating the same machinery. If a GUI workflow needs behavior that the CLI cannot inspect, validate, or replay, add the CLI surface first or in the same slice. The GUI may provide richer visual controls, but it should not become a second source of rules, parsing, mutation, or import/export semantics.

## Do Not Reinvent Existing Rust Backend Work

The Rust backend/data pipeline built to support the original Python-facing tool is the reliable substrate. The new Rust GUI/frontend rewrite is the part that needs stricter review, simplification, and parity cleanup. Before creating a new pipeline or schema, check these Rust-owned modules:

- `crates/mole_ledger`: parity ledger registry and CLI/GUI surface contract.
- `crates/mole_cli`: agent-facing commands for decomp search, frame data, generated artifacts, replay traces, packaging, and parity status.
- `crates/mole_devtool`: native egui shell, GUI view models, tables, move keyframe preview/editor.
- `crates/mole_runtime`: runtime rendering snapshots, Slippi/core trace diagnostics, generated runtime source frame data.
- `crates/mole_core`: deterministic gameplay state, source units, collision, motion, and engine import targets.
- `crates/mole_frame_data`: compact runtime source capsule decoding.

Repurpose and extend backend/data surfaces first. Prefer adding a typed CLI/report/apply surface before wiring a GUI control, then let the GUI consume that same Rust model. Only translate legacy Python GUI behavior when the Rust GUI lacks the capability.

For GUI layout, sizing, color, selection, and panel behavior, follow `docs/architecture/rust-devtool-ui-primitives.md`. Move tab fixes into shared primitives when they are reusable; do not let a one-tab patch become the copied pattern for the next surface.

## Lossless Artifact Requirements

Every middleware artifact that can feed the engine should carry:

- `schema_version` and artifact kind.
- Target character/state/runtime identity.
- Source character/action/table identity.
- Raw decomp values and converted Rust/runtime values when both exist.
- Source paths, line numbers, offsets, symbols, handlers, or procedure refs.
- Projection and unit conversion policy.
- Known gaps and confidence notes.
- Overrides/edits as explicit data, not silent replacement.
- Import/export status and generated runtime output paths.

If a GUI edit cannot be represented in the artifact and replayed through the CLI, do not treat it as durable.

## Move Keyframes And Source Imports

The current `Move Keyframes` GUI workflow is a browser first:

```text
target character -> target state -> frame/keyframe view -> details/provenance
```

It should match the old Python frontend shape where useful: compact top selectors, primary preview/timeline on the left, and state/frame details on the right. The Rust GUI should list both materialized state artifacts like `resources/melee/frame_data/dolphin_mole/AttackAirN.json` and compact manifest-only actions from `resources/melee/frame_data/<target>/source_manifest.json`.

Manifest-backed states must not appear as blank just because no editable artifact exists. The GUI loads them through the same Rust sampler path used by CLI frame-data sampling, currently `mole_frame_data::sample_action_keyframes`, and projects the sampled hit capsules, hurt capsules, frame counts, active windows, and provenance into the normal `MoveKeyframesSurface`. These sampled views remain read-only and keep `artifact_path` empty until an explicit materialized artifact or typed override workflow exists.

Runtime state-name and source-action mapping are core primitives. Use `mole_core::motion_state_for_runtime_variant`, `mole_core::runtime_motion_state_for_source_key`, `mole_core::RUST_MOTION_STATE_VARIANTS`, and `mole_core::CANONICAL_SOURCE_ONLY_ACTION_BINDINGS` from both CLI and GUI code rather than adding tab-local or command-local variant tables. Source-only Melee actions like `Attack12` may have a source action-table id that differs from the canonical runtime action id; the GUI and CLI must resolve those through the shared core mapping before asking the runtime for baked capsules.

Import and replacement workflows should wrap the existing Mole CLI frame-data commands rather than creating a GUI-only path. Examples:

- `mole frame-data extract --character dolphin_mole --source-character marth --state AttackLw3 --source-state AttackLw3 --write --json`
- `mole frame-data extract --character dolphin_mole --source-character peach --state AttackLw4 --source-state AttackLw4 --write --json`
- `mole frame-data extract --all-states --character dolphin_mole --source-character marth --write --json`
- `mole frame-data export-runtime --all-states --character dolphin_mole --output crates/mole_runtime/src/generated/source_frame_data.rs --write --json`

If a future GUI dropdown needs to show source characters, source actions, import plans, or profile-value replacement candidates such as Luigi friction values, add a CLI inspection/dry-run surface first or in the same slice. Useful future agent-facing commands include:

- `frame-data catalog`: list target characters and their materialized/manifest states.
- `frame-data source catalog`: list extractable source characters and source actions from extracted decomp resources.
- `frame-data import-plan`: dry-run target/source character and state mappings before writing artifacts.
- `value import-plan` or equivalent: dry-run typed profile/global/stage value imports while preserving int/float ownership and provenance.

Do not add visible GUI import controls until they call the same Rust-owned command/view-model path that the CLI exposes.

## Runtime-Backed GUI Editing

For visual editors, the rendered scene is owned by the Rust engine/runtime. GUI overlays may annotate that scene, but they must not become a second renderer or a second source of geometry truth.

Move-keyframe preview is an engine viewport, not a standalone GUI renderer. The current path is `World/RenderFrame -> RenderScene::from_frame_on_stage(..., StageProfile::dev_flat_test(), ...) -> egui/SDL draw`. The devtool may choose the viewport stage and camera, but the scene geometry, sprite cue, hurt capsules, hit capsules, and ECB polygon come from the runtime scene. Draw the sprite rectangle only as an image target/fallback; never treat the rectangle as body geometry when runtime capsules or the ECB are available.

The visible GUI should remain browser/read-only until editing can be represented by typed runtime primitives and replayed through the CLI. Existing editor-model handle code is limited to runtime-aligned collision primitives that are already represented in the rendered scene or active engine artifact path, such as ECB/body-volume points and hitbox/hurtbox capsule endpoints. Imported figatree/JObj skeleton data remains preserved source metadata until the runtime exposes typed pose/joint edit primitives. Do not reintroduce raw JSON joint dragging or fake rig overlays as durable behavior; add typed runtime pose primitives first, then expose matching CLI validation and GUI controls in the same slice.

## Feature Acceptance Checklist

For every new parity feature, add or update all of these:

1. Typed Rust schema or view model.
2. CLI command or subcommand.
3. GUI surface or panel.
4. Engine importer/exporter when the data affects runtime.
5. Validation or diff test.
6. Documentation entry in the relevant workflow doc.

The expected implementation shape is:

```text
schema + CLI + GUI + engine import/export + tests + docs
```

## Current Rust Migration Baseline

Rust already owns important backend pieces:

- The parity ledger registry and generated map.
- Active value sheets for global, character, physics, combat, and stage values.
- Native Rust GUI shell in `mole_devtool`.
- Move keyframe runtime preview, character/state browser, compact source-manifest awareness, and model-level collision handle editing tests.
- Decomp search/show/symbol CLI commands.
- Frame data extract/sample/export-runtime CLI commands.
- Slippi replay/core trace diagnostics.

Remaining parity work should close gaps on top of these pieces instead of replacing them.

## Legacy Reference Surfaces

Use these Python files only to understand visual/interaction behavior that still needs to be translated:

- `tools/state_graph_viewer.py`: old dev tool UI behavior.
- `RealMainFile.py`, `states.py`, `Camera.py`, `DebugOverlay.py`, `DisplayInputs.py`: old playable Pygame harness behavior.

Do not add new responsibilities to those files.
