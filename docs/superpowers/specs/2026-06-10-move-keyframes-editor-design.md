# Move Keyframes Editor Design

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this design task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Current status:** Superseded by the 2026-06-11 browser/catalog parity slice. This document is historical design context for future editing work, not the active visible-GUI target.

**Current goal:** Keep `Move Keyframes` as a character/state browser and runtime preview over materialized artifacts plus compact source manifests. Visible editing resumes only after the runtime exposes typed edit primitives and the Mole CLI can inspect, apply, validate, and export the same edits.

**Historical architecture note:** The original editor sketch centered on `resources/melee/frame_data/dolphin_mole/AttackAirN.json` and one reusable geometry primitive. That model-level handle code remains useful as test coverage and future reference, but it should not be exposed as durable visible GUI editing until the CLI-backed edit contract exists.

**2026-06-11 update:** The original sketch allowed raw joint dragging and visible direct geometry editing. That is now superseded. Imported figatree/JObj skeleton data is preserved as provenance and inspection data, but joint handles and other durable edit controls should not be visible until `mole_runtime` exposes typed pose/joint primitives and the Mole CLI can validate the same edit surface.

**Tech Stack:** Rust, `eframe`/`egui`, `serde_json`, existing `mole_devtool` data loaders, existing frame-data JSON artifact.

---

## Problem Statement

The current Rust `Move Keyframes` tab should first behave like a browser/import foundation: select a target character, select a state, inspect the materialized or manifest-backed frame data, and preview materialized frames through the runtime scene. The long-term goal is still a lightweight keyframe editor, but that goal waits for CLI-backed typed edit primitives. Pose/joint editing remains a later typed-runtime primitive, not a raw JSON overlay.

We want the editor to:
- stay Rust-first,
- avoid a large file explosion,
- reuse one geometry-editing core for all editable shapes,
- keep the preview visually simple,
- and preserve a clean path to later export/compile steps.

## Design Principles

- One editor core for all editable geometry.
- One artifact shape first, not a new parallel animation format.
- One canvas, one future handle system, one future CLI-backed save/export path.
- Keep the visual language close to the current Python viewer, but do not reintroduce Python as the authority.
- Prefer explicit override/apply records over hidden model mutation once editing returns.

## Editor Model

The editor should expose a single editable scene derived from the current keyframe artifact:

- selected frame index,
- frame list,
- editable geometry handles,
- selection state,
- current drag state,
- dirty/clean state,
- and serialized export state.

Shape kinds should normalize into a shared handle model:

- `CapsuleEndpointHandle`
- `CapsuleRadiusHandle`
- `BodyVolumeHandle`
- `EcbHandle`

All active handles should be draggable through the same interaction plumbing, even if they affect different JSON fields underneath. A future `PoseJointHandle` must be added only after the runtime exposes the same typed primitive the GUI draws.

## Rendering Model

The tab should stay split into two areas:

- left side: a frame list / keyframe table,
- right side: a canvas preview of the currently selected frame.

The canvas should draw:
- the runtime-rendered fighter/pose representation,
- ECB/body volume outlines,
- hurtboxes,
- hitboxes,
- and selected handles.

The preview should remain intentionally plain. The goal is editability and parity with the existing Python viewer, not a cinematic renderer.

## Interaction Model

Interactions should be minimal and predictable:

- click a row to select a frame,
- hover a handle to highlight it,
- drag a handle to edit the underlying geometry,
- release to commit the drag,
- keyboard shortcuts can come later,
- undo/redo should be data-focused, not tool-focused.

The interaction layer should not know about save/export mechanics beyond setting the dirty flag when edits occur.

## Save and Export

The first save/export path should write back to the current frame-data JSON shape.

That means:
- edited geometry is persisted in the same artifact layout the tab already loads,
- the export path remains a single, explicit action,
- and later a compiler step can convert the edited JSON into a final animation-set format without changing the editor UI.

This is deliberate:
- it keeps the current tool honest,
- it avoids a premature format fork,
- and it gives us a stable modular base for future animation-set compilation.

## Error Handling

- If the artifact cannot be loaded, the tab should show a readable failure state instead of panicking.
- If a frame lacks a geometry field, the editor should skip that handle rather than invent one.
- If an edit cannot be applied cleanly, the drag should cancel and the original geometry should remain intact.
- If export fails, the UI should keep the dirty state and show the error text inline.

## Testing Strategy

The editor needs coverage at three levels:

- load tests for the current keyframe artifact,
- interaction tests for selecting and dragging geometry handles,
- export tests that confirm the JSON round-trips with the expected fields intact.

The tests should prove that:
- the selected frame changes correctly,
- dragging a handle mutates the correct geometry,
- the preview can still render after edits,
- and save/export produces valid JSON.

## Out of Scope for This Slice

- A runtime engine snapshot editor.
- A new animation file format.
- A compiler that emits the final future animation set.
- Special-case UI per shape type.

Those can come later, but the editor should not depend on them.

## Superseded Acceptance Criteria

- Current visible GUI acceptance is browser parity: character selector, state selector, runtime preview for materialized frames, compact timeline, right-side state details, and bounded responsive panels.
- Capsule endpoints and ECB/body-volume controls remain future editing work until exposed through typed runtime primitives and CLI validation/apply commands.
- Joint controls are absent unless backed by typed runtime pose primitives and matching CLI validation.
- Edits are not considered durable unless saved through an explicit artifact/override path that the CLI can replay.
- Future editor work must stay small and modular, with one shared geometry-editing core.
- The Rust devtool remains the primary path; Python is not expanded.
