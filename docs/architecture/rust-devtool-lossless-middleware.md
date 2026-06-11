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

## Runtime-Backed GUI Editing

For visual editors, the rendered scene is owned by the Rust engine/runtime. GUI overlays may annotate that scene, but they must not become a second renderer or a second source of geometry truth.

Move-keyframe editing currently uses `RenderFrame -> RenderScene` for the viewport. Editable GUI handles are limited to runtime-aligned collision primitives that are already represented in the rendered scene or active engine artifact path, such as ECB/body-volume points and hitbox/hurtbox capsule endpoints. Imported figatree/JObj skeleton data remains preserved source metadata until the runtime exposes typed pose/joint edit primitives. Do not reintroduce raw JSON joint dragging or fake rig overlays as durable behavior; add typed runtime pose primitives first, then expose matching CLI validation and GUI controls in the same slice.

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
- Move keyframe runtime preview and editable handles.
- Decomp search/show/symbol CLI commands.
- Frame data extract/sample/export-runtime CLI commands.
- Slippi replay/core trace diagnostics.

Remaining parity work should close gaps on top of these pieces instead of replacing them.

## Legacy Reference Surfaces

Use these Python files only to understand visual/interaction behavior that still needs to be translated:

- `tools/state_graph_viewer.py`: old dev tool UI behavior.
- `RealMainFile.py`, `states.py`, `Camera.py`, `DebugOverlay.py`, `DisplayInputs.py`: old playable Pygame harness behavior.

Do not add new responsibilities to those files.
