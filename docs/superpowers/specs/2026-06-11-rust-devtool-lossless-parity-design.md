# Rust Dev Tool Lossless Parity Design

Date: 2026-06-11

## Purpose

Bring the Rust dev tool to parity with the previous Python-facing dev tool while preserving the larger contract: decomp reference, Rust engine, Mole CLI, and Rust GUI must remain aligned.

The Rust dev tool is the editable middleware between decomp extraction and engine import. It must not become a second interpretation of the data. It must use the same Rust-owned artifacts and typed view models that the CLI uses.

## Design Principles

- Rust is the durable implementation surface.
- Python/Pygame is reference-only.
- CLI and GUI are dual surfaces.
- Artifacts are lossless middleware, not display-only caches.
- Existing Rust backend/data crates should be repurposed before new systems are created.
- The current Rust GUI rewrite is provisional frontend code, not a trusted architecture; it should be simplified and corrected around the backend contracts.
- Editing must preserve provenance, source-space values, conversion policy, and explicit overrides.

## Architecture

The shared source of truth is a Rust-owned artifact graph:

```text
Decomp/reference files
  -> mole_cli extraction/generation
  -> typed Rust artifacts and view models
  -> mole_devtool GUI
  -> mole_cli validation/import/export
  -> mole_core/mole_runtime consumption
```

`mole_ledger` records which subsystem tabs exist and whether CLI/GUI surfaces are active or planned. `mole_cli` provides agent-facing access. Runtime import/export lives in `mole_runtime`, `mole_frame_data`, and `mole_core`. `mole_devtool` should be a thin, faithful GUI frontend over those backend contracts, not a parallel data model.

The Rust GUI should be built from small primitives that are easy to reuse for new tabs: tab registration, responsive split panes, spreadsheet/table views, detail panes, refreshable artifact selectors, runtime preview panes, and edit/save affordances. These primitives should stay minimal and data-driven. A new tab should usually provide a typed backend surface plus a small adapter into existing UI primitives, not a new bespoke renderer.

Reusable GUI rules are recorded in `docs/architecture/rust-devtool-ui-primitives.md`. When a layout or formatting fix is useful beyond one tab, promote it into `layout.rs`, `template.rs`, `theme.rs`, or another focused primitive before copying it. The current Move Keyframes browser is a consumer of that workbench pattern, not the owner of a special one-off layout.

The theme is a shared primitive, not per-tab styling. Status colors and workbench identity colors live in `crates/mole_devtool/src/theme.rs`, with table/status rendering consuming that palette. If a tab needs a new semantic color, add it to the shared palette with a test that names the Python reference or Rust runtime source it preserves.

## Required Parity Areas

### State Graphs

Rust must regain the interactive graph canvas from the Python viewer: two graph panes, status colors, node/edge details, pan/zoom, draggable layout, linked Melee/Mole equivalent movement, edge label pinning, curved edge lanes, legend, and save layout.

### Parity Ledger

Rust already has active value tabs. It needs cleaner table behavior, stable row selection, better horizontal scrolling, source/provenance inspection, and no duplicated status display.

### ECB Coverage

Rust has structured rows. It still needs refresh, source artifact visibility, missing/unmapped lists, and import-readiness status.

### Input Trace

Rust must replace hard-coded fixture windows with selectable input exports, players, frame windows, refresh, and structured/raw views.

### Slippi Replay

Rust must expose replay trace selection and first-diff navigation through both CLI and GUI. Trace windows must produce rows after filtering by requested source frames.

### Move Keyframes

Rust already has a runtime preview/editor. It needs to become the move editor's primary visual surface: render the actual Rust game/runtime scene on a basic single-floor stage, then let selected keyframes or frame-step controls move the animation forward and backward one frame at a time. The preview should show the current pose, hitboxes, hurtboxes, body volumes, and ECB using the same runtime render scene path that the engine consumes where possible.

It also needs full character/state population, sampled compact-manifest frames, all-shape editing instead of first-shape-only editing, explicit override records, and CLI parity for each durable edit.

The runtime scene is the visual authority. The editor overlay is only an editable annotation layer over artifact handles: selected joint, pill endpoint, hitbox center, or body/ECB point. Do not draw a separate pose-tree rig as though it were the runtime pose unless the data comes through the runtime render path or is explicitly marked as a diagnostic overlay.

The inspector should edit the selected handle first, not dump the whole artifact into one panel. The stable workflow is frame selection -> handle selection -> focused value editing -> save through the same artifact path the CLI validates.

### Playable Runtime Visual Reference

The old Pygame game UI should be translated only where the Rust runtime lacks a corresponding visual or usability affordance: main menu identity, background/stage/sprite presentation, stocks, shield visual, ECB overlays, FPS/debug overlay, pause overlay, resize/fullscreen behavior.

## Acceptance Criteria

- Every GUI feature has a matching CLI command or report.
- Every CLI extraction/diff/import path has a visible GUI status.
- Every engine-fed artifact preserves raw source data, converted runtime data, provenance, and overrides.
- `cargo test -p mole_devtool` passes.
- New or changed shared behavior is tested in the crate that owns the shared logic.
- Docs point future workers to Rust-first workflows and legacy Python only as reference.
