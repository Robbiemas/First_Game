# Runtime Baked Data, Performance, And Portability Contract

Date: 2026-06-11

This document records the approved architecture rule for extracted Melee data after it passes through the Mole CLI/devtool middleware into the Rust engine. Read this before changing extraction, generated runtime data, renderer hot paths, rollback snapshots, or engine-facing asset formats.

## North Star

Mole is translating Melee-shaped source behavior into a compact Rust engine, not building a large editor database that the runtime drags around.

The source path is:

```text
decomp / ISO reference
  -> Mole CLI extraction and validation
  -> editable middleware artifact when needed
  -> generated compact Rust runtime artifact
  -> deterministic core/runtime consumption
```

The CLI and devtool may hold provenance, editor metadata, comments, source offsets, raw JSON, and richer inspection views. The runtime must consume the compact generated form needed for gameplay, rollback, rendering, and diagnostics.

## Four Pillars

### Low Latency

Runtime frame work must be predictable. The local play path should avoid per-frame allocations, blocking file IO, raw DAT parsing, JSON parsing, dynamic source lookup, or debug drawing unless explicitly enabled.

Diagnostics are allowed, but they should be opt-in or bounded. A debug overlay, trace writer, or parity report must not quietly become part of the normal play loop cost.

### Rollback

Authoritative state must remain compact and deterministic. Rollback snapshots should store the smallest state needed to restore and replay whole 60 Hz simulation frames.

Generated source data may be referenced by stable identifiers and static tables. It should not be copied into every snapshot. Rendering caches, editor state, raw source provenance, strings, file paths, and debug logs do not belong in rollback-owned state.

### Performance

The runtime should do only the work required for the current frame. Prefer static tables, borrowed baked slices, value types, fixed-size arrays, and cache-friendly structs over clone-heavy or allocation-heavy paths.

The first optimization is architecture: keep extraction offline, bake runtime data once, and keep hot paths simple. Do not add a complex framework or cache unless measurement shows the simple compact path is insufficient.

### Size Parity

Extracted data must not bloat when it becomes engine data. If a source table, stage blob, frame-data slice, or common fighter accessory is extracted from the decomp or ISO, the generated runtime representation should remain in the same order of magnitude as the source data needed by gameplay.

Do not expand compact source data into large editor-friendly runtime structures. If expansion is useful for editing, keep it in CLI/devtool middleware artifacts and generate a smaller engine form.

Red flags:

- A few kilobytes of source table becomes hundreds of kilobytes of runtime data without a measured reason.
- Runtime stores both raw source JSON and generated Rust tables.
- Per-frame code clones generated vectors instead of borrowing static slices.
- Runtime keeps source provenance strings in every gameplay object.
- Rollback snapshots include render/debug/editor-only fields.

## Runtime Data Rules

Generated runtime artifacts should be:

- Source-backed: every value can be traced to decomp/ISO extraction or an explicit typed override.
- Compact: store only fields needed by core/runtime behavior and normal rendering.
- Static where possible: baked tables should be accessible without heap allocation.
- Borrowable: hot paths should read slices and value types without cloning owned collections.
- Engine-agnostic: data should sit below SDL, egui, Unity, Godot, or any future shell.
- Deterministic: no runtime parsing, filesystem probing, timestamp dependency, or random ordering inside authoritative gameplay.

Middleware artifacts may be larger because they serve humans and agents. They should not be linked into runtime unless the runtime genuinely needs that data.

## Extraction And Baking Boundary

Use the CLI to extract, verify, diff, and generate. The game should not need the ISO, decomp checkout, raw DAT files, or research folder after data has been accepted and baked.

Allowed at CLI/devtool time:

- DAT/root parsing.
- Source offset preservation.
- JSON inspection artifacts.
- Dry-run import plans.
- Source-to-target mapping reports.
- Human-readable provenance.
- Editor handles and draft overrides.

Allowed at runtime:

- Compact generated Rust tables.
- Stable source identifiers.
- Numeric values in source units.
- Baked collision, camera, stage, frame, accessory, and state data.
- Optional bounded diagnostics under explicit flags.

Not allowed in normal runtime:

- Reading raw DAT files.
- Parsing JSON source artifacts during play.
- Looking into `.research`.
- Depending on decomp/ISO files.
- Converting editor schemas into gameplay structures every frame.

## Portability Boundary

The Rust backend should remain suitable for another shell or engine later. SDL3 is the current native runtime shell, and egui is the current devtool shell, but core/runtime data formats should not assume either.

Keep these layers separate:

```text
mole_core: deterministic simulation and compact source-backed data types
mole_runtime: render scene construction, local shell integration, diagnostics
mole_cli: extraction, validation, generation, packaging
mole_devtool: GUI over the same CLI/runtime/core concepts
external engine adapter: future Unity/Godot/other shell over compact Rust APIs
```

If a feature only works because SDL or egui owns the data, the boundary is wrong. The shell may display or drive the feature, but the source-backed runtime primitive should live below the shell.

## Vetted Pattern From The Latency Pass

The approved direction from the 2026-06-11 latency cleanup:

- Legacy sprite paths are static string literals instead of newly allocated per-frame strings.
- Runtime source hit/hurt capsule tables are borrowed from baked data instead of cloned per frame.
- Runtime source frame data is baked as one Rust module plus three canonical
  sidecars: `source_frame_capsules.bin`, `source_figatree_bundle.bin`, and
  `source_manifest.json`. Do not reintroduce loose per-action `.figatree.bin`
  sidecars; index the packed bundle through generated metadata instead.
- Local SDL debug overlay construction is opt-in through `--debug-overlay`.
- The runtime consumes generated data and does not parse `PlCo.dat`, stage DAT files, or JSON extraction artifacts during play.

These are examples of the expected shape. Future changes should follow the same principle: move work to extraction/generation time, keep the normal frame loop compact, and make diagnostics explicit.

## Review Checklist

Before accepting a new extraction, runtime, stage, character, frame-data, camera, or renderer change, answer these questions:

1. Does the runtime consume baked compact data instead of raw source/editor artifacts?
2. Is the generated runtime size close to the source data actually needed by gameplay?
3. Are source provenance and editor metadata kept outside hot rollback/render state?
4. Does the normal frame loop avoid new heap allocation, file IO, and debug drawing?
5. Does rollback snapshot only authoritative deterministic state?
6. Can the same compact data be used by another shell later?
7. Can the CLI inspect, validate, regenerate, or diff the data without the GUI?
8. Is any larger representation justified by a measured runtime need?

If the answer to any question is no, pause and reshape the boundary before adding more features.
