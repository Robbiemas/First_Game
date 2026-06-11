# Rust Dev Tool UI Primitives

Date: 2026-06-11

This document is the reusable UI pattern guide for the native Rust dev tool. Read it before adding a new tab, reformatting an existing tab, or copying a layout fix from one surface to another.

## Rule

Do not solve layout, sizing, color, selection, or artifact-control problems as single-tab hacks. If a fix is generally useful, move it into a primitive or document it as a primitive rule before using it twice.

The GUI should feel like one workbench built from small Rust-owned parts:

```text
typed backend surface
  -> top controls
  -> summary/status strip
  -> bounded primary/detail body
  -> optional data/detail subtabs on narrow widths
```

## Current Primitive Owners

- `crates/mole_devtool/src/layout.rs`
  - `responsive_split_layout`: the default left/right panel primitive. It keeps wide windows in a 50/50 row and stacks narrow windows so panels do not drift off-screen.
  - `bounded_child_height`: the default child-height primitive for text/table panes that may need internal scrolling.
- `crates/mole_devtool/src/template.rs`
  - `render_spreadsheet_table`: the default structured table primitive. It owns semantic row status rendering, row selection, detail text, bounded vertical height, and responsive column widths.
- `crates/mole_devtool/src/theme.rs`
  - Shared workbench colors and semantic status colors. Do not add per-tab palettes unless the semantic color belongs in the shared theme.
- `crates/mole_devtool/src/app.rs`
  - App-level selection state, active tab state, and reload/select methods. UI functions should call small app methods instead of mutating a backend model in several places.
- `crates/mole_devtool/src/ui.rs`
  - Thin egui rendering adapters. Keep tab-specific code here shallow; put reusable behavior in `layout.rs`, `template.rs`, `theme.rs`, or a typed surface module.

## Workbench Surface Pattern

Use this layout for browser/editor surfaces unless there is a strong domain reason not to:

1. Top row: compact controls such as artifact path, target character, state, player, frame window, refresh, or save.
2. Summary strip: short source/status text from the typed surface, not a paragraph explaining the UI.
3. Primary body:
   - Wide layout: `responsive_split_layout` with primary content on the left and details/inspector on the right.
   - Narrow/mobile layout: sub-tabs such as `Preview`, `Details`, and `Table` instead of stacking every panel vertically.
4. Data-heavy pane: shared sheet or bounded scroll area with `bounded_child_height`.
5. Durable action: call the same Rust model or CLI-backed command path that agents use.

## What Counts As A Primitive Fix

Promote a fix out of a single tab when it affects any of these:

- Panel splitting or stacking.
- Width/height bounds.
- Table column sizing.
- Row selection and details.
- Artifact picker and refresh controls.
- Status colors.
- Save/apply controls.
- Runtime preview sizing.
- CLI/GUI dual-surface state.

The Move Keyframes dense timeline is currently tab-specific because it represents frame numbers. The responsive sheet columns are shared because every table can overflow.

## No-Off-Screen Rule

No widget should assume a fixed application width or height. When space is tight:

- Split panes stack or become sub-tabs.
- Tables compress columns before they request horizontal space.
- Dense timelines paint into the available rectangle rather than allocating one widget per frame.
- Long raw text and huge tables use bounded internal scrolling.
- Whole-tab vertical scrolling is a fallback for data inspection, not the main layout strategy.

Avoid hidden global scrollbars where a domain-specific sub-tab, bounded table, or compact visualization would preserve context better.

## CLI And GUI Parity

The UI primitive layer does not need a CLI command by itself. User-visible workflows built with the primitives do.

Before adding a durable GUI operation, make sure the Mole CLI can inspect, dry-run, apply, validate, or export the same behavior. If agents would benefit from discovering a GUI-backed data source, add a CLI catalog or dry-run command in the same slice.

Current planned agent-facing helpers:

- `frame-data catalog`: target characters plus materialized and manifest-only states.
- `frame-data source catalog`: extractable source characters/actions.
- `frame-data import-plan`: dry-run target/source state replacement.
- Typed value import plans for profile/global/stage values, preserving source integer/float ownership.

## Runtime Viewports

Engine-owned visuals should enter the GUI as runtime viewports. The reusable shape is:

```text
typed selection -> RenderFrame -> RenderScene on an explicit StageProfile -> GUI draw adapter
```

The GUI draw adapter may cache textures and decide how to fit the viewport rectangle, but it must not invent gameplay geometry. Sprites are visual assets, hit/hurt capsules and ECB polygons are runtime scene primitives, and body rectangles are fallback image targets only. Collision primitives should be drawn as wireframes in editor viewports so the user can inspect the engine-owned geometry without filled circles, blobs, or fake body blocks. Use `StageProfile::dev_flat_test()` for compact editor previews unless the workflow explicitly selects another stage.

When engine work lands in another branch, devtool tabs should update by consuming the changed `mole_core` and `mole_runtime` contracts. A new visual/editor feature should therefore start as a shared runtime or CLI primitive, then get a GUI adapter. Avoid tab-local replicas of engine state, camera math, collision geometry, animation stepping, or stage setup; those replicas will drift and break lossless parity.

## Tests

Shared primitives own their own tests. When a tab exposes a new reusable behavior:

- Add or update a primitive test in the owning module.
- Add a small app/surface test only for tab-specific selection or loading behavior.
- Do not prove a primitive solely through a screenshot or one tab's visual code path.

Examples already in place:

- `layout::tests::responsive_split_layout_stacks_narrow_graphs_without_overflow_width`
- `layout::tests::bounded_child_height_never_exceeds_available_space`
- `template::tests::responsive_columns_fit_inside_narrow_available_width`
- `template::tests::responsive_columns_scale_below_minimums_when_the_panel_is_tiny`

## Anti-Patterns

- Copying a panel layout from one tab into another without extracting or documenting the shared shape.
- Fixed widths that exceed the current panel.
- Fixed heights that exceed the current body.
- Per-tab theme toggles or local color schemes.
- Fake renderers for engine-owned data.
- GUI-only mutation paths.
- Showing unfinished controls that do not call the typed backend or CLI-backed command.
