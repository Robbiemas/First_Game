# Rust Dev Tool Lossless Parity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring the Rust dev tool, Mole CLI, Rust engine, and decomp extraction pipeline into a dual-surface, lossless parity workflow.

**Architecture:** Reuse the existing Rust backend/data pipeline as the source of truth. `mole_cli` and `mole_devtool` must consume the same typed artifacts and view models, while engine import/export remains in `mole_runtime`, `mole_frame_data`, and `mole_core`. Treat the current Rust GUI rewrite as provisional frontend code that can be simplified or replaced module-by-module when it obscures those contracts.

**Tech Stack:** Rust 2021, `mole_cli`, `mole_devtool`, `mole_ledger`, `mole_runtime`, `mole_core`, egui/eframe, serde JSON artifacts.

---

### Task 1: Stabilize Current Rust Dev Tool Tests

**Files:**
- Modify: `D:/Mole Game/First_Game/crates/mole_runtime/src/slippi_diagnostic.rs`
- Modify: `D:/Mole Game/First_Game/crates/mole_devtool/src/app.rs`

- [x] Fix `trace_slippi_export_from_match_start_with_core` so `max_frames` caps emitted trace rows after source-frame filtering, not the first raw export frames before filtering.
- [x] Update stale app-state expectations so an aligned graph with zero missing entries is a passing state.
- [x] Run `cargo test -p mole_devtool`.

### Task 2: Add Rust-First Workflow Documentation

**Files:**
- Modify: `D:/Mole Game/First_Game/README.md`
- Modify: `D:/Mole Game/First_Game/AGENTS.md`
- Create: `D:/Mole Game/First_Game/docs/architecture/rust-devtool-lossless-middleware.md`

- [x] Link the lossless middleware contract from natural entry points.
- [x] State that Python/Pygame is reference-only.
- [x] Record the CLI/GUI dual-surface rule.

### Task 3: Restore State Graph Canvas Parity In Rust

**Files:**
- Modify/Create focused modules under `D:/Mole Game/First_Game/crates/mole_devtool/src/`
- Use existing graph JSON and `config/state_graph_layout.json`.

- [x] Add a Rust view model that loads both Melee reference and Mole current graphs.
- [x] Render two side-by-side egui graph canvases with status colors matching the Python palette.
- [ ] Add pan/zoom, node selection, edge selection, and detail panes.
- [ ] Add node dragging and save-layout support.
- [ ] Add edge label visibility and pinning with linked equivalent edge behavior.
- [x] Add CLI parity for graph layout inspection if missing.
- [ ] Add CLI parity for graph layout save validation if missing.

### Task 3A: Tighten Reusable Rust GUI Primitives

**Files:**
- Modify: `D:/Mole Game/First_Game/crates/mole_devtool/src/template.rs`
- Modify or split from: `D:/Mole Game/First_Game/crates/mole_devtool/src/ui.rs`

- [x] Keep table rendering data-driven and reusable across tabs.
- [x] Treat row status as a semantic attribute; do not duplicate a visible Status column when the source table already has one.
- [ ] Extract repeated panel patterns only when at least two current tabs use them.
- [ ] Prefer small primitives for tab bars, summary strips, table/detail panes, artifact selectors, and runtime previews.
- [ ] Avoid broad GUI frameworks inside the crate; keep egui wrappers thin and obvious.

### Task 4: Make Trace And Replay Surfaces Selectable

**Files:**
- Modify: `D:/Mole Game/First_Game/crates/mole_cli/src/replay.rs`
- Modify: `D:/Mole Game/First_Game/crates/mole_devtool/src/input_trace.rs`
- Modify: `D:/Mole Game/First_Game/crates/mole_devtool/src/slippi_replay.rs`
- Modify: `D:/Mole Game/First_Game/crates/mole_devtool/src/ui.rs`

- [x] Add typed Rust devtool load options for explicit input export path, player, start frame, end frame, and max rows where applicable while preserving the current default artifact.
- [x] Add CLI commands or options for listing available trace/replay artifacts if absent.
- [x] Add GUI controls for artifact path, player, start frame, end frame, and refresh.
- [x] Preserve structured table and raw/report text views.
- [x] Add first-diff navigation for replay traces.

### Task 5: Complete Move Keyframes As Lossless Middleware

**Files:**
- Modify: `D:/Mole Game/First_Game/crates/mole_devtool/src/move_keyframes.rs`
- Modify: `D:/Mole Game/First_Game/crates/mole_devtool/src/ui.rs`
- Modify: `D:/Mole Game/First_Game/crates/mole_cli/src/frame_data.rs`
- Modify runtime export/import modules as needed.

- [ ] Populate character and state selectors from artifact discovery instead of single current values.
- [ ] Preserve live sampling from compact source manifests.
- [x] Make the selected-frame viewport render the Rust runtime scene on a basic single-floor stage rather than a standalone diagram.
- [x] Add frame-step controls so clicking keyframes or stepping arrows moves the runtime pose and volumes forward and backward one frame at a time.
- [ ] Edit every hitbox, hurtbox, body volume, and pose joint, not just the first item.
- [ ] Store edits as explicit overrides where possible.
- [ ] Add CLI commands that can inspect, apply, validate, and export the same edits.
- [ ] Verify extract -> edit -> export-runtime -> import path does not drop provenance or source-space data.

### Task 6: Translate Runtime Visual Affordances Only Where Rust Lacks Them

**Files:**
- Prefer existing Rust runtime render/input/debug modules.
- Use old Python/Pygame files only as visual references.

- [ ] Audit Rust runtime for menu, background/stage/sprite, stocks, shield, ECB, FPS/debug, pause, resize/fullscreen parity.
- [ ] Add missing Rust runtime UI features with tests or smoke checks where feasible.
- [ ] Do not add new Python behavior.

### Task 7: Maintain The Dual-Surface Ledger

**Files:**
- Modify: `D:/Mole Game/First_Game/crates/mole_ledger/src/lib.rs`
- Modify: `D:/Mole Game/First_Game/crates/mole_cli/src/ledger_map.rs`
- Modify: `D:/Mole Game/First_Game/crates/mole_devtool/src/lib.rs`

- [x] Any new CLI capability updates the ledger registry.
- [x] Any new GUI capability updates the same registry.
- [x] `docs/state_graphs/parity_ledger_map.json` remains the generated shared contract.
- [x] Add tests that fail when CLI and GUI surface states drift.

### Verification

Run these before claiming a parity slice is complete:

```powershell
cargo fmt --check
cargo test -p mole_devtool
cargo test -p mole_cli
cargo test -p mole_runtime
cargo run -p mole_cli -- generated write-ledger-map --json
```
