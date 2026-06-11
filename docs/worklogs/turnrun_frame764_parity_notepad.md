# TurnRun frame-764 parity notepad

Purpose: a short working note for the current parity investigation so I can resume quickly after compaction.

## Current status

- Focused contract test passes: `turn_run_completion_into_run_uses_source_run_phys_handoff_tick`.
- Replay trace for player 2 source frames `760..768` now matches state and
  horizontal velocity exactly.
- Confirmed trace:
  - Frames `760..763` match exactly.
  - Frame `764`:
    - expected `Run (21)`, expected `ground_x 2148`
    - actual `Run`, actual `velocity 2148`
    - delta `0`
  - Frames `765..768` stay aligned through `KneeBend`.

## Important source facts already confirmed

- `ftCo_TurnRun_Anim` calls `fn_800CA644(gobj)` when the animation ends.
- If the run-stick gate passes, it enters Run via `ftCo_Run_Enter(gobj, p_ftCommonData->x430)`.
- `x430` is `mv.co.run.x0` / the Run no-interrupt counter, not the Run
  animation start frame. `ftCo_Run_Enter` still passes `anim_start = 0.0F` to
  `Fighter_ChangeMotionState`.
- `ftCo_Run_Enter_Full` stores `mv.co.run.x0 = arg0` and `mv.co.run.x4 = fp->gr_vel`.
- `ftCo_Run_Anim` uses `mv.co.run.x4` when the ground friction multiplier is below `1.0`, otherwise `gr_vel`.
- `ftCo_TurnRun_Phys` and `ftCo_Run_Phys` both use `gr_friction * p_ftCommonData->x60_someFrictionMul`.
- `ftCo_Run_Phys` plus `ftCommon_8007C98C` does not clamp toward the Run
  target when `fp->gr_vel * accel < 0`; in that branch it stages `accel`
  directly.
- The missing frame-764 clamp comes earlier, inside
  `Fighter_ChangeMotionState`: `animflags_bool` is captured from the old
  action flags, the new Run action flags are loaded, and the source bridge
  clamps `gr_vel` to `co_attrs.dash_run_terminal_velocity` when leaving a
  root-motion action for a non-root-motion action.
- Source action animation flags from
  `resources/melee/frame_data/dolphin_mole/source_manifest.json`:
  - `TurnRun`: `0x80000082`
  - `Run`: `0x40000002`
  - Falcon `dash_run_terminal_velocity`: `2.299999952316284`

## What I changed so far

- Added `StageSurface.friction_multiplier: f32`.
- Added `floor_friction_multiplier_for_bottom(stage, bottom)` helper.
- Wired `apply_ground_traction` and `run_ground_friction` to use the stage multiplier.
- Threaded `stage` through the relevant sim helpers.
- Updated Battlefield test stage literals to include `friction_multiplier: 1.0` for now.

## 2026-06-10 devtool sheet template and ECB Coverage slice

- Extracted a shared Rust sheet primitive in `crates/mole_devtool/src/template.rs` so the parity-ledger rows can be rendered through one reusable table shell instead of copy-pasted UI logic.
- Kept the spreadsheet-style color coding and light/dark theme support inside that shared template so future tabs can inherit the same visual grammar.
- Ported the Rust devtool's `ECB Coverage` section onto the same template path using the existing `docs/state_graphs/parity_reports/falcon_ecb_coverage.json` artifact.
- Added a Rust-owned `EcbCoverageSurface` model and wired it into `ParityLedgerApp` so the section is loaded at app startup instead of being a placeholder.
- The devtool now has a reusable sheet pattern that can be applied to the remaining sections without expanding the Python viewer path.
- Verified `cargo check -p mole_devtool` passes after the refactor.

## 2026-06-10 move keyframes Rust slice

- Ported the `Move Keyframes` section onto the same Rust sheet template using the existing `resources/melee/frame_data/dolphin_mole/AttackAirN.json` artifact.
- Added a Rust-owned `MoveKeyframesSurface` model so the tab can show a real table of keyframes, counts, and interpolation status instead of a placeholder.
- The tab now uses the same reusable grid and theme palette as the parity ledger and ECB coverage sheets, keeping the front end on one consistent primitive.
- Verified `cargo check -p mole_devtool` passes after the new section was added.

## 2026-06-10 state graphs Rust slice

- Ported the `State Graphs` section onto the same Rust sheet template using the existing `docs/state_graphs/mole_current_graph.json` artifact.
- Added a Rust-owned `StateGraphsSurface` model that surfaces the missing node/edge rows as a readable sheet instead of leaving the tab as a placeholder.
- The section now shares the same table shell and theme palette as the parity ledger, ECB coverage, and move-keyframes tabs.
- Verified `cargo check -p mole_devtool` passes after the new section was added.

## 2026-06-10 input trace Rust slice

- Ported the `Input Trace` section onto the same Rust sheet template using the existing `debug/slippi/Game_20260530T214929.inputs.json` export.
- Added a Rust-owned `InputTraceSurface` model that filters the current parity window (`760..768`) for player 2 and shows raw plus Rust-normalized input values in a single table.
- The section keeps the same sheet styling and color palette as the other Rust devtool tabs, so the UI is still one reusable primitive instead of a special-case panel.
- Verified `cargo check -p mole_devtool` passes after the new section was added.

## 2026-06-10 slippi replay Rust slice

- Ported the `Slippi Replay` section onto the same Rust sheet template using the runtime trace generated from the current input export window.
- Added a Rust-owned `SlippiReplaySurface` model that loads the `replay trace` comparison for player 2 over frames `760..768` and exposes expected vs actual state, position, and velocity comparisons as a table.
- The final placeholder tab is now gone, so the top-level Rust devtool shell is entirely data-backed by the shared sheet primitive.
- Verified `cargo check -p mole_devtool` passes after the new section was added.

## 2026-06-11 trace/replay selectable backend slice

- Added explicit Rust devtool load options for `InputTraceSurface` and `SlippiReplaySurface` so the backend can load a caller-selected input export, player, frame window, and replay max-row cap instead of only the default `debug/slippi/Game_20260530T214929.inputs.json` fixture.
- The default devtool loaders still target the current parity window for compatibility, but the new option path matches the existing CLI `replay trace --inputs --player --start --end --frames` contract.
- Verified the focused devtool tests:
  - `cargo test -p mole_devtool input_trace_loads_explicit_artifact_player_and_window`
  - `cargo test -p mole_devtool slippi_replay_loads_explicit_artifact_player_and_window`
- Next trace/replay slice should add visible GUI selector controls and CLI artifact discovery/listing, then wire first-diff navigation for replay rows.

## 2026-06-11 trace/replay selectable GUI and CLI slice

- Added read-only CLI artifact discovery as `cargo run -p mole_cli -- replay artifacts --json`; it lists existing `debug/slippi/*.inputs.json` exports and `replays/*.slp` / `*.slp.gz` files without exporting or mutating anything.
- Added the command to the CLI help catalog so agent-facing discovery and parser behavior stay aligned.
- Added Input Trace and Slippi Replay GUI controls for artifact path, player, start frame, end frame, refresh, replay max rows, and replay first-diff navigation.
- The GUI controls reload through the same Rust surface option structs that the tests cover, keeping the hard-coded default fixture as compatibility behavior only.
- Corrected the stale future-ledger design text that still described the Python viewer as the current devtool renderer; the Rust dev tool is now the human-facing path and Python is historical/reference-only.
- Verification:
  - `cargo fmt --check`
  - `cargo test -p mole_devtool`
  - `cargo test -p mole_cli`
- Remaining architectural gap: `mole_ledger` still primarily models value-ledger tabs, while the Rust devtool shell now has additional active surfaces (`State Graphs`, `ECB Coverage`, `Input Trace`, `Slippi Replay`, `Move Keyframes`). The next registry slice should decide whether to broaden the registry from value-ledger tabs into a full devtool surface registry or add a sibling surface registry, then make CLI/GUI drift tests cover these active sections.

## 2026-06-11 full devtool surface registry slice

- Broadened the Rust-owned `mole_ledger` contract with `devtool_surfaces`, a sibling registry for the six active outer Rust devtool sections:
  - `State Graphs`
  - `Parity Ledger`
  - `ECB Coverage`
  - `Input Trace`
  - `Slippi Replay`
  - `Move Keyframes`
- Each devtool surface now declares CLI commands, GUI section identity, source artifacts, outputs, status, and CLI/GUI access state.
- `docs/state_graphs/parity_ledger_map.json` was regenerated through `cargo run -p mole_cli -- generated write-ledger-map --write --json`, so the checked-in artifact is again generated from Rust rather than hand-edited.
- `ParityLedgerViewModel` now carries `devtool_surfaces`, and `ParityLedgerApp` has a test that fails if visible GUI sections drift from active registry surface labels.
- Backward compatibility note: `LedgerMap` deserializes older maps with missing `devtool_surfaces` as an empty list, so old artifacts fail softly instead of breaking basic parsing.
- Verification:
  - `cargo fmt --check`
  - `cargo test -p mole_ledger`
  - `cargo test -p mole_devtool`
  - `cargo test -p mole_cli`

## 2026-06-11 documentation unity cleanup

- Corrected older active specs/plans that still described Python/Tk/Python viewer paths as current, acceptable, or bridge renderers.
- Unified the docs rule: Rust is the durable CLI/devtool/engine path; Python is historical/reference-only except for temporary legacy extraction support while Rust replacements are added.
- Left historical research notes that describe past Python behavior as evidence, but removed forward-looking guidance that would cause agents to extend Python UI work.
- Scan used:
  - `rg -n "current Tk|current dev tool still|still renders through Python|Python remains acceptable|Python viewer is still the bridge|current durable human-facing path" docs/superpowers docs/architecture README.md AGENTS.md`

## 2026-06-11 trace/replay raw report view slice

- `InputTraceSurface` now retains the raw source input-export JSON text so the Rust GUI can expose the exact artifact behind the structured table.
- `SlippiReplaySurface` now builds a trace report text block from the loaded comparison rows, giving the Rust GUI a report-style view alongside the spreadsheet.
- The `Input Trace` and `Slippi Replay` tabs now expose collapsible raw/report panes without changing the underlying artifact format.
- This completes the current Task 4 trace/replay selectable-surface checklist: CLI artifact discovery, GUI selectors, refresh controls, structured rows, raw/report text, and first-diff navigation are all present.
- Verification:
  - `cargo fmt --check`
  - `cargo test -p mole_devtool`

## 2026-06-10 move keyframes visual restore

- Restored a live preview pane to the `Move Keyframes` tab so the selected keyframe renders again instead of showing only the spreadsheet rows.
- The preview uses the actual frame geometry from `resources/melee/frame_data/dolphin_mole/AttackAirN.json`, drawing body volumes plus hitbox and hurtbox capsules for the selected frame.
- The tab now behaves as a split view: table on the left, visual preview on the right, with the same theme palette and shared sheet primitive still underneath.
- Verified `cargo check -p mole_devtool` passes after the preview render was restored.

## 2026-06-10 move keyframes editor design and plan

- Wrote the design doc for the next `Move Keyframes` slice at `docs/superpowers/specs/2026-06-10-move-keyframes-editor-design.md`.
- Wrote the implementation plan at `docs/superpowers/plans/2026-06-10-move-keyframes-editor-implementation.md`.
- The editor direction is now a single Rust geometry-editing core for runtime-aligned capsules and ECB/body volumes, with save/export staying on the current JSON artifact shape first. Raw joint editing is superseded until runtime-owned pose primitives exist.
- Next execution should start from the implementation plan, keeping Python out of the new editor path.

## 2026-06-10 resolved frame-764 source path

- The previous stage-friction and Run_Phys-target-clamp notes were stale.
- The matching source path is:
  1. Frame 763 post is `TurnRun` with `gr_vel`/reported ground speed
     `2.788125`.
  2. `ftCo_TurnRun_Anim` completes and `fn_800CA644` enters Run when the
     source `x58` run-stick gate passes.
  3. `Fighter_ChangeMotionState` sees the old TurnRun root-motion action flags
     and the new Run non-root-motion flags, then clamps `gr_vel` to Falcon
     `dash_run_terminal_velocity` (`2.3` source units).
  4. Same-frame `Run_Phys` applies the current frame opposite-stick accel
     (`-0.1525`) through `ftCommon_8007C98C`, producing `2.1475`.
- Rust now mirrors that with a compact source-action flag table in core and a
  motion-change ground-velocity bridge on Run entry. The actual gameplay math
  stays in source `f32`; milli integers remain compatibility/readout output.
- Verification:
  - `cargo test -p mole_core --test core_contract turn_run_completion_into_run_uses_source_run_phys_handoff_tick -- --nocapture`
  - `cargo run -p mole_cli -- replay trace --inputs debug\\slippi\\Game_20260530T214929.inputs.json --player 2 --start 760 --end 768 --format markdown`
- Cargo note: if the repo-local `target` directory is locked by another agent,
  use a writable external target such as
  `CARGO_TARGET_DIR=C:\\Users\\Robie\\.codex\\memories\\mole_target_turnrun_core_test`.
  Keep one `cargo test` name filter per invocation.

## Python parity ledger boundary

- The Python-based parity ledger remains useful as an evidence index because it
  currently exposes extra source-backed values and categories before all of that
  surface is translated to Rust.
- It is not a runtime dependency and should not become the gameplay authority.
  Use the Python ledger to identify source facts, confirm them against decomp /
  extracted DAT artifacts, then bake the minimal compact data into Rust core,
  Rust CLI, or Rust devtool surfaces.

## 2026-06-09 Rust ledger foundation update

- Pivoted this session from the frame-764 TurnRun parity bug to the parity-ledger foundation work the user requested.
- Implemented a new Rust-owned value sheet generator at `crates/mole_cli/src/value_sheets.rs`.
- Added Mole CLI command: `cargo run -p mole_cli -- generated write-value-sheets --write --json`.
- Rewired generated-artifact metadata so the repo now points at the Rust generator path instead of `tools/generate_value_sheets.py` for the value-sheet group.
- Generated five Rust-owned ledger outputs:
  - `docs/state_graphs/value_sheets/global_common_values.json`
  - `docs/state_graphs/value_sheets/captain_falcon_values.json`
  - `docs/state_graphs/value_sheets/physics_engine_values.json`
  - `docs/state_graphs/value_sheets/combat_physics_values.json`
  - `docs/state_graphs/value_sheets/battlefield_stage_values.json`
- Superseded on 2026-06-11: `battlefield_stage_values.json` is now sourced from the `StageProfile::battlefield()` compatibility projection over `MeleeStageProfile::battlefield()`, with the full extracted MapCollData wireframe kept in `resources/melee/extracted/stages/battlefield_stage.json`.
- This keeps Battlefield represented as an explicit stage ledger example instead of implying that the engine has only one spiritual default stage.

## Deferred on purpose

- The Python state graph viewer was not extended with new tabs in this slice.
- The full dev-tool migration from Python to Rust is still outstanding; this session only moved ledger generation onto a Rust-owned foundation.
- The Captain Falcon ditto frame-764 TurnRun -> Run parity bug is still an active separate frontier and should resume after this tooling slice.

## Resume point

1. Decide whether the next slice is:
   - Rust dev-tool viewer / parity-ledger UI work, or
   - returning to frame-764 TurnRun parity debugging.
2. If continuing the tool migration, build the Rust-side ledger consumer around these five generated sheets rather than adding more Python-specific ledger behavior.
3. If returning to parity debugging, keep the new ledger sheets available as reference material but do not let this tooling work replace source-backed motion-state investigation.

## 2026-06-09 Battlefield stage pipeline slice

- Implemented a Rust CLI stage-asset writer for `battlefield`, exposed as `generated write-stage-asset --stage battlefield --write --json`.
- The generated stage blob now lives at `resources/melee/extracted/stages/battlefield_stage.json`.
- The stage asset is generic in shape and intended as the template for future stage extraction, but it is currently sourced from the Rust Battlefield stage profile rather than a live decomp pipeline.
- Generated-check metadata now points at Rust generator modules for both value sheets and the new stage asset path.
- Verified the new CLI contract with targeted tests:
  - `generated_write_stage_asset_emits_battlefield_stage_blob`
  - `generated_check_reports_missing_and_stale_artifact_groups`
  - `generated_check_markdown_summarizes_groups_and_commands`

## Next frontier

- The remaining architectural gap is the real stage extraction source path: the CLI can write a Battlefield stage asset, but the engine still builds Battlefield from the hand-authored Rust stage profile rather than consuming a generalized extracted stage blob.
- The next slice should connect that stage asset shape back into the core stage boundary in a way that stays generic for additional stages.

## 2026-06-09 viewer tab fix

- The dev tool viewer was still hardcoded to the original two value-sheet files, which is why the new tabs were not visible when opened.
- Updated `tools/state_graph_viewer.py` so the parity ledger now loads and renders:
  - Global Values
  - Test Character Values
  - Physics Engine Values
  - Combat Physics Values
  - Stage Values
- Added a dedicated read-only stage sheet tab so Battlefield stage data can be inspected in the viewer even though it is not a direct decomp-to-Rust comparison table yet.
- Updated the viewer tests to reflect the five-sheet ledger shape.

## 2026-06-09 value-sheet coverage audit

- Re-ran a source-to-sheet coverage comparison across:
  - `plco_common_data.json`
  - `captain_falcon_profile.json`
  - `global_common_values.json`
  - `captain_falcon_values.json`
  - `physics_engine_values.json`
  - `combat_physics_values.json`
  - `battlefield_stage_values.json`
- Result: `133 / 133` extracted source fields are represented across the current sheet set.
- Physics engine coverage is complete at `36` fields.
- Combat physics coverage is complete at `30` fields after adding the previously missed damage / hitlag / L-cancel / passive-response fields.
- Battlefield stage coverage is complete for the current Rust stage asset shape:
  - main floor
  - soft platforms
  - blast zones
  - spawn points
  - friction multipliers
- Updated the viewer test expectations to the new partitioned counts so the suite matches the Rust-generated ledger split.

## 2026-06-09 dual-surface parity ledger framework

- Added a Rust-owned parity ledger registry crate at `crates/mole_ledger`.
- The registry now models tabs as subsystem surfaces with an explicit dual-surface rule:
  - every tab has `cli` and `gui` access states
  - the roadmap registry is considered healthy only when the CLI and GUI states match
- Added a Rust CLI writer for the canonical registry map:
  - `cargo run -p mole_cli -- generated write-ledger-map --write --json`
- The checked-in ledger map now lives at `docs/state_graphs/parity_ledger_map.json`.
- The new ledger map is intended to be the shared contract for both:
  - agentic/CLI inspection and generation
  - future human GUI consumption
- Future ledger or dev-tool changes should keep the Rust registry as the source of truth and only use Python as historical reference or temporary legacy extraction support, not as the authority.

## 2026-06-09 owned ledger-map consumer bridge

- Added a typed Rust consumer shape for the parity ledger map in `crates/mole_ledger`.

## 2026-06-10 move keyframes editor tree slice

- The Rust move-keyframes editor now loads the imported figatree / JObj skeleton from `resources/melee/extracted/captain_falcon_costume_skeleton.json`.
- `MoveKeyframesEditorSurface` now exposes the imported tree so the devtool can stay aligned with the decomp hierarchy instead of treating joints as anonymous points.
- The app shell now owns the same editor object, and the `Move Keyframes` tab shows the figatree root plus joint count as a lightweight sanity signal.
- The core editor state keeps 3D joint positions in Rust and only flattens at render time, which preserves the backend shape for future engine integrations.
- Performance note: the skeleton is loaded once at app startup, not parsed per frame, so the move-keyframes path stays simple and should remain responsive.

## 2026-06-10 move keyframes joint drag slice

- Superseded on 2026-06-11 by the runtime-backed edit rule. Raw pose JSON joint dragging was removed from the active editor because those handles were not owned by the runtime scene and could drift from the hurtbox/collision display.
- The imported figatree/JObj skeleton remains preserved source metadata, loaded once for inspection and provenance.
- Joint editing should return only after `mole_runtime` exposes typed pose/joint primitives that the CLI can validate and the GUI can manipulate without a second geometry authority.

## 2026-06-10 move keyframes save round-trip slice

- Added a Rust save path that round-trips the existing `AttackAirN.json` shape without introducing a new animation format.
- `MoveKeyframesSurface::load_from` now supports arbitrary JSON paths so save/export can be tested against temp files.
- The editor clears dirty state only after a successful write, which keeps the save behavior predictable and cheap.
- This preserves the current artifact shape as the export contract, which is important for future Unity / Godot backends.

## 2026-06-10 move keyframes handle interaction slice

- The `Move Keyframes` preview now draws draggable handle markers for the selected frame.
- As of 2026-06-11, active draggable handles are limited to runtime-aligned collision geometry: hitbox endpoints, hurtbox endpoints, and ECB/body-volume points.
- The active handle state is kept tiny: one active handle plus accumulated screen delta, which keeps the interaction path responsive.
- The save button is now exposed in the Rust UI and writes back to the current artifact path when the editor is dirty.

## 2026-06-10 move keyframes collision-pill drag checkpoint

- The shared drag helper also works for hurtbox endpoints, which means the same low-overhead editor path covers the collision-pill style geometry too.
- The editor still mutates in place and preserves the 3D data model in Rust, so the UI remains just a thin view layer over the core data.

## 2026-06-10 move keyframes verification pass

- Ran the full focused move-keyframes test filter: `cargo test -p mole_devtool move_keyframes_ -- --nocapture`.
- Historical result: all move-keyframes editor tests passed at the time, including skeleton loading, joint drag, hurtbox drag, and save round-trip.
- Superseded result: the current accepted baseline excludes raw joint dragging and keeps skeleton loading as provenance only until the runtime exposes typed pose handles.
- The consumer can round-trip the registry into an owned `LedgerMap` with:
  - tab metadata
  - surface access states
  - summary counts
- The checked-in `docs/state_graphs/parity_ledger_map.json` artifact is now written in a form that Rust can load directly.
- This keeps the future GUI path aligned with the Rust contract instead of forcing a separate interpretation layer in Python.

## 2026-06-09 parity surface integration for ledger map

- The Rust CLI parity surfaces now consume the owned ledger map directly:
  - `parity`
  - `parity snapshot`
  - `agent brief`
- Those reports now surface the full `LedgerMap` object, including the nested registry counts and tab list, instead of treating the file as a passive generated artifact only.
- `recommended_next` now flags missing or non-dual-surface ledger maps so the macro plan can keep the CLI/GUI contract honest.

## 2026-06-09 completion gate ledger check

- `finish check` now includes a dedicated `ledger map` completion gate check.
- The completion gate fails if the owned ledger map is missing or loses CLI/GUI dual-surface parity.
- This keeps the highest-level CLI handoff check aligned with the Rust-owned parity contract instead of letting the ledger map drift quietly out of date.

## 2026-06-09 Rust devtool view-model layer

- Added a shared Rust crate at `crates/mole_devtool` for the GUI-ready dev-tool view model.
- The crate turns the owned `LedgerMap` into a `ParityLedgerViewModel` with:
  - registry counts
  - tab rows
  - per-tab CLI/GUI surface alignment
- Added a Rust CLI surface for the model:
  - `cargo run -p mole_cli -- devtool ledger --json`
- The CLI `devtool ledger` surface is read-only and loads the same parity ledger map that the agentic commands already consume.
- This is the first real Rust dev-tool shell/view-model seam, and it keeps the path toward a native GUI fully Rust-owned.

## 2026-06-09 native Rust devtool shell

- Added a native Rust GUI shell in `crates/mole_devtool` that loads `docs/state_graphs/parity_ledger_map.json` and renders the owned `ParityLedgerViewModel`.
- The shell shows the subsystem tabs directly, with a left-side tab list and a selected-tab detail pane.
- The recommended human launch path is now:
  - `cargo run -p mole_devtool`
- The Python state-graph viewer is still present as historical reference, but it is no longer the authoritative devtool path and should not receive new ledger behavior.

## 2026-06-10 move keyframes runtime-backed preview

- The `Move Keyframes` editor preview now uses the actual Rust runtime scene path instead of the bespoke custom canvas.
- The preview scene is built from `RenderScene::from_frame(...)` and renders the runtime engine order: background, stage surfaces, players, hurtboxes, hitboxes, shields, and ECB.
- Editor handles still overlay the runtime scene and drag back into the same editable keyframe JSON.
- Handle motion now inverts the runtime transform scale so drag deltas stay one-to-one with the engine view.

## 2026-06-10 move keyframes runtime preview simplification

- The move-keyframes viewport now renders the runtime-backed scene with only the selected fighter's relevant layers: stage, player body, ECB, hurtboxes, and hitboxes.
- The preview camera is framed around the selected frame's collision cluster so the character is centered more intentionally instead of sitting on the raw battlefield camera.
- The old bespoke preview canvas helpers were removed after the runtime-backed path stabilized, keeping the devtool lean.

## 2026-06-10 move keyframes viewport plus timeline layout

- The `Move Keyframes` tab is now organized as a modular stack: runtime viewport first, then a horizontal keyframe strip, then a collapsible full data sheet.
- Frame tiles in the strip are color-coded for attack frames and selection state, so future edit and export work can keep the timeline readable at a glance.
- The lower collapsible sheet stayed as the exposed value surface for the full frame data at this checkpoint; later browser/catalog work moved durable editing behind typed runtime primitives and CLI-backed validation.

## 2026-06-10 move keyframes debug stage and inspector

- The move-keyframes preview now uses a dedicated flat debug stage instead of Battlefield, with only one visible ground surface in the scene.
- The preview viewport now fit-factors the rendered runtime geometry so the character and collision shapes stay centered and zoomed appropriately.
- The imported joint hierarchy is not drawn as a default editable overlay; it is provenance data until the engine owns the typed pose primitive.
- A visible selected-frame inspector was added under the timeline strip so keyframe metadata and hitbox/hurtbox/body-volume values remain exposed outside the click-drag handles.
- Float-based engine values should stay float-based in the runtime/editor path; only frame indices and other discrete counters stay integral.

## 2026-06-10 move keyframes right-facing overlay and direct inspector

- Superseded on 2026-06-11: the right-facing raw joint overlay was removed because it was not the same primitive the engine renders.
- Runtime-aligned handle dragging remains for collision geometry visible in the `RenderScene` viewport.
- The frame strip compresses horizontally to fit the window instead of turning into a scroll wheel.
- Superseded on 2026-06-11: the right-hand direct-edit panel was replaced by a state/frame details panel until typed runtime edit primitives and CLI-backed validation exist.
- The Rust devtool continues to repurpose existing editor/runtime code rather than rewriting the wheel.
- The viewport height is now tighter so the selection/inspector region can stay visible in the same window without clipping.

## 2026-06-11 functional handoff cleanup

- The Rust devtool now has one shared theme palette module for workbench identity colors and status colors; table rendering consumes the shared palette instead of carrying duplicate row colors.
- The devtool uses one Python-viewer-style workbench palette; the visible theme toggle is removed so new tabs inherit one consistent baseline.
- The move-keyframes viewport keeps `RenderFrame -> RenderScene` as the visual authority and no longer draws the imported pose-tree rig as a default overlay.
- Editable handles remain as the annotation layer over the runtime scene. Selecting a runtime-aligned collision handle exposes focused values in the right inspector, and dragging/editing the selected handle updates the projected artifact `x/y` fields.
- The old `Open State Graphs.cmd` entrypoint was replaced by `Open Dev Tool.cmd`; runtime launcher contracts now point at the renamed Rust devtool launcher.
- Verification for this handoff:
  - `cargo fmt --check`
  - `cargo test -p mole_devtool`
  - `cargo test -p mole_cli`
  - `cargo test -p mole_runtime`
  - `cargo test -p mole_core`

## 2026-06-11 state graph layout contract

- Added a typed Rust state graph canvas model in `mole_devtool` that loads both `docs/state_graphs/melee_reference_graph.json` and `docs/state_graphs/mole_current_graph.json`.
- The model applies `config/state_graph_layout.json` so Rust sees the same saved node positions and zoom values the Python viewer used.
- Added graph validation for duplicate/missing nodes, missing statuses, missing roots, and edges that point at absent nodes.
- Added `mole graph layout --json` as a read-only CLI surface over the same Rust model, reporting both graph panes, node/edge counts, zoom, status counts, and validation errors.
- Updated the dual-surface ledger so State Graphs now lists `graph layout` alongside `graph missing`, `graph next`, and `graph inspect`.
- Verification for this slice:
  - `cargo test -p mole_devtool`
  - `cargo test -p mole_cli`

## 2026-06-11 state graph read-only canvas

- The Rust devtool now renders the Melee reference graph and Mole current graph as two side-by-side egui canvases.
- The panes consume the shared `StateGraphCanvasPair` model and use the Python-derived semantic status palette, keeping the GUI tied to the same data contract as `mole graph layout --json`.
- The existing missing-entry spreadsheet remains below the canvases so visual graph parity and audit-table parity stay visible in one tab.
- This is intentionally a read-only parity slice. Pan/zoom, node and edge selection, dragging, save-layout, and edge label pinning remain open in the plan.
- Verification for this slice:
  - `cargo fmt --check`
  - `cargo test -p mole_devtool`

## 2026-06-11 state graph interaction canvas

- Added a small Rust-owned state graph interaction model for per-graph canvas view state, zoom clamping, pan offsets, node hit-testing, edge hit-testing, and selection detail formatting.
- The State Graphs tab now preserves a view for each graph pane, exposes zoom/reset controls, supports middle/right-drag panning, and lets the user click nodes or edges to inspect their parity metadata.
- Selected nodes and edges are highlighted directly on the canvas while the detail pane below the graph panes shows source/status/value metadata from the same graph document model.
- Node dragging, layout save, edge label visibility, and edge label pinning remain separate open plan items.
- Verification for this slice:
  - `cargo fmt --check`
  - `cargo test -p mole_devtool`

## 2026-06-11 state graph layout editing

- Added shared Rust layout save/validation support to `StateGraphCanvasPair`, including node-position mutation and canonical writes back to `config/state_graph_layout.json`.
- Added `mole graph layout save --check|--write` as the CLI-side twin for validating or intentionally writing the same layout contract used by the GUI.
- The State Graphs tab now supports primary-dragging nodes and a Save Layout control that writes only the layout file, leaving the graph JSON documents untouched.
- Regenerated `docs/state_graphs/parity_ledger_map.json` so State Graphs lists `graph layout save` alongside the other CLI commands.
- Edge label visibility and pinning remain open; this slice covers node dragging and layout persistence only.
- Verification for this slice:
  - `cargo fmt --check`
  - `cargo test -p mole_devtool`
  - `cargo test -p mole_cli`
  - `cargo run -p mole_cli -- generated write-ledger-map --write --json`

## 2026-06-11 responsive Rust devtool layout foundation

- Added shared Rust layout primitives for bounded split panes and bounded child heights, with tests proving narrow panes stack and no child requests more height than the window offers.
- Moved the State Graphs tab toward the old Python frontend structure: header controls stay at the top, graph canvases remain the primary split-pane body, and selection/missing-entry surfaces are accessed through sub-tabs instead of being stacked below the canvases.
- Move Keyframes now uses the same responsive split primitive: desktop keeps preview/timeline on the left and state details on the right, while narrow/mobile widths switch between Preview, Details, and Table sub-tabs.
- Reusable sheets and raw text panes now use bounded heights instead of fixed values, preserving internal scrolling only for data-heavy surfaces where virtualization/scrolling is the performance-friendly option.
- Architecture rule going forward: new Rust devtool tabs should reuse `layout.rs` primitives and prefer sub-tabs over overflowing panels; virtualized/bounded data panes are preferred over rendering off-screen widgets.
- Verification for this slice:
  - `cargo fmt --check`
  - `cargo test -p mole_devtool`

## 2026-06-11 move keyframes browser/catalog parity

- Recentered the `Move Keyframes` tab on the current intended workflow: choose a target character, choose a state from that character's frame-data pool, inspect the frame/keyframe data, and defer editing until the runtime/CLI have typed edit primitives.
- Added a Rust frame-data catalog for the GUI that lists known target characters and both materialized state artifacts plus manifest-only actions from `source_manifest.json`.
- The Rust GUI now follows the old Python frontend structure more closely: top selectors, runtime preview plus dense timeline on the left, and state/frame details on the right. Narrow widths use Preview, Details, and Table sub-tabs instead of stacked panels that can run off-screen.
- The keyframe strip is now a painted, full-width timeline. It compresses to the panel width, colors active hitbox/keyed/selected frames, and selects the nearest materialized keyframe on click.
- The shared sheet primitive now computes responsive column widths instead of fixed totals, so table-based tabs have a common bounded-width rule.
- Added `docs/architecture/rust-devtool-ui-primitives.md` to make this a reusable workbench pattern instead of a Move-Keyframes-only fix. Future tabs should reuse the layout, sheet, theme, and bounded sizing primitives before adding bespoke panel code.
- Visible fake rig/joint editing remains removed. The browser can load manifest-only states like `AttackLw3` without pretending they are editable artifacts.
- Confirmed the existing CLI import/extract path already supports target/source mappings such as Marth down tilt into Dolphin Mole down tilt through `frame-data extract --character <target> --source-character <source> --state <target-state> --source-state <source-state> --write`.
- Recorded next useful CLI surfaces in `docs/architecture/rust-devtool-lossless-middleware.md`: frame-data catalog, source catalog, import-plan dry run, and typed value import-plan.
- Corrected the Move Keyframes ledger summary so the generated dual-surface registry describes browsing, compact-manifest inspection, runtime preview, and export readiness rather than visible direct editing.
- Verification for this slice:
  - `cargo fmt --check`
  - `cargo test -p mole_devtool`
  - `cargo test -p mole_ledger -p mole_devtool -p mole_cli`
  - `cargo run -p mole_cli -- generated write-ledger-map --write --json`

## 2026-06-11 move keyframes manifest sampling fix

- Fixed the blank-state path when selecting compact manifest states such as `AttackLw3`.
- `MoveKeyframesSurface::load_for_character_state` now falls back from missing materialized artifacts to `mole_frame_data::sample_action_keyframes`, the same Rust sampler family used by CLI frame-data sampling.
- Manifest-backed views now populate sampled frame rows, active hitbox/hurtbox windows, and provenance while keeping `artifact_path` empty so they remain read-only browser views until typed edit/export primitives exist.
- The runtime preview binding now follows the selected surface's runtime motion state and source binding instead of hardcoding `AttackAirN`, so changing states advances the real runtime scene for the chosen state.
- Reduced duplicate state-name plumbing by promoting runtime `MotionState` lookup, source-action aliases, and the variant list into `mole_core`; the CLI and GUI now share the same primitive.
- Regression coverage:
  - `cargo test -p mole_devtool move_keyframes_loads_manifest_state_as_sampled_read_only_browser_view`
  - `cargo test -p mole_devtool app_move_keyframe_selectors_load_manifest_states_as_sampled_read_only_views`

## 2026-06-11 move/state runtime viewport cleanup

- Removed another duplicate source-action path by promoting canonical source-only action bindings into `mole_core`. CLI runtime export and GUI previews now share `CANONICAL_SOURCE_ONLY_ACTION_BINDINGS`, so source-only actions such as `Attack12` resolve from Melee source table id `47` to canonical runtime action id `45` instead of accidentally rendering `Attack100Start`.
- Added `StageProfile::dev_flat_test()` and `RenderScene::from_frame_on_stage(...)` so the move/state editor viewport asks the runtime for a scene on an explicit flat dev stage instead of building a frame on one stage and patching a Battlefield-rendered scene afterward.
- Updated the egui viewport to draw the runtime sprite cue from the project PNG assets, then runtime hurt capsules, hit capsules, and ECB polygon over it. The player rectangle is now only a fallback image target when there is no sprite and no runtime collision data.
- Fixed the SDL renderer to honor `LegacySpriteCue::flip_x`, keeping the game renderer and devtool viewport aligned.
- Ran the requested Captain Falcon all-states CLI refresh for Dolphin Mole:
  - `cargo run -q -p mole_cli -- frame-data extract --all-states --character dolphin_mole --source-character captain --write --json`
  - `cargo run -q -p mole_cli -- frame-data export-runtime --all-states --character dolphin_mole --output crates/mole_runtime/src/generated/source_frame_data.rs --write --json`
  - Extract summary: `mapped_state_count=275`, `runtime_mapped_state_count=65`, `manifest_bytes=11116070`.
  - Runtime export summary: `state_count=105`, `figatree_chunk_count=99`, `hitbox_count=1000`, `hurtbox_count=39644`, `generated_bytes=5194838`.
- Regression coverage:
  - `cargo test -p mole_devtool move_keyframes_source_only_manifest_state_uses_canonical_runtime_binding`
  - `cargo test -p mole_devtool move_keyframe_preview_uses_sprite_asset_instead_of_player_rect_when_available`
  - `cargo test -p mole_runtime render_scene_can_use_dev_flat_stage_without_battlefield_surfaces`

## 2026-06-11 laptop merge and wireframe viewport follow-up

- Fast-forwarded the current branch to `origin/laptop/test`, bringing in the laptop agent's `feat: align source down wait stand input` work before continuing local devtool work.
- Re-ran the Captain Falcon all-states import/export after the merge. The Dolphin Mole compact manifest still reports `mapped_state_count=275`, `runtime_mapped_state_count=65`, and the runtime export reports `state_count=105`, `figatree_chunk_count=99`, `hitbox_count=1000`, `hurtbox_count=39644`.
- Added a GUI catalog regression proving `list_move_keyframe_states("dolphin_mole")` exposes every unique state in `source_manifest.json` exactly once. Current state count is `275`.
- Changed the move/state editor viewport adapter to render runtime capsules as wireframes instead of filled pills: side strokes, endpoint rings, and small endpoint markers. The ECB polygon is now stroked without a filled body block.
- Regression coverage:
  - `cargo test -p mole_devtool move_keyframes_catalog_exposes_every_manifest_state_once`
  - `cargo test -p mole_devtool move_keyframe_capsules_are_projected_as_wireframe_sides_not_filled_pills`

## 2026-06-11 engine/devtool viewport coupling guard

- User clarified that another agent may be changing the game engine in parallel. The devtool must reflect those engine changes through shared `mole_core` and `mole_runtime` types, because the devtool viewport is only an adapter over the runtime engine scene.
- Added a regression proving the move/state editor preview scene is exactly the engine-built `RenderScene::from_frame_on_stage(&preview.frame, StageProfile::dev_flat_test(), ...)`, with only editor-specific entry platform suppression applied afterward.
- Architecture rule: no tab-local replicas of engine state, camera math, collision geometry, animation stepping, or stage setup. Add shared runtime/CLI primitives first, then adapt them in the GUI.

## Future Note: State Animation Batching After Parity

- User wants full lossless parity first: every source state should remain importable and inspectable while combat, game flow, and the runtime loop are still being translated from the decompilation.
- After parity is proven, revisit small duplicated state animations such as repeated item/swing variants. If multiple states are byte-for-byte or semantically equivalent under the typed runtime model, they can be batched/deduplicated behind a canonical source mapping.
- Do not collapse or alias these states now. The future batching pass must preserve source action IDs, provenance, CLI import/export behavior, GUI selection, and round-trip losslessness.

## 2026-06-11 compact import reset and GUI import panel

- Cleared the old Dolphin Mole per-state frame-data cache and refilled it through the Rust CLI all-states Captain Falcon import/export pipeline. Dolphin Mole now keeps `resources/melee/frame_data/dolphin_mole/source_manifest.json` as the compact authority; the stale expanded `AttackAirN.json` cache is intentionally removed.
- Added a Move/State editor import panel beside the target selector. It defaults to importing all Captain Falcon states into Dolphin Mole by running the same Mole CLI commands agents use: `frame-data extract --all-states` followed by `frame-data export-runtime --all-states`.
- Updated devtool and CLI tests to treat manifest-backed states as the normal full-import browser path. The manifest-backed surface now exposes the embedded Captain Falcon skeleton path so rig metadata remains available after removing expanded per-state JSON.
- Updated the egui viewport adapter to draw runtime ECB polygons as the decomp-shaped top/right/bottom/left diamond with top-bottom and left-right axes. The ECB data still comes from `mole_runtime::RenderScene`, not a GUI-local body box.

## 2026-06-11 import responsiveness and collision visibility follow-up

- User observed that `Import All States` looked frozen while Cargo was running. Root cause: the GUI button executed the CLI import/export synchronously on the egui frame. The button now starts a background worker, disables itself as `Importing...`, and polls for completion before refreshing the state catalog.
- User observed states such as `Swing42` reporting hurtboxes but not visibly drawing them. Root cause: editor wireframes reused runtime alpha, which was too faint over the light sprite/background. The devtool now keeps the runtime RGB/type color but forces editor capsule overlays to full opacity.
- Capture/hold naming check: `CaptureHoldLw` is not a compact-manifest state key. Current imported Captain Falcon keys include `CapturePulledLw`, `CaptureWaitLw`, and `CaptureDamageLw`; `CaptureWaitLw` samples 11 hurt capsules. Do not fake a missing `CaptureHoldLw` state unless the decomp/source table proves that is the correct canonical name.

## 2026-06-11 Python parity ledger compatibility note

- The other workstation revived the Python parity ledger because that ledger view is still more usable than the Rust parity ledger. This is accepted as a temporary compatibility surface, not a reversal of the Rust-owned architecture.
- Added `execs/Open Python Parity Ledger.cmd` beside `execs/Open Dev Tool.cmd`. Future merges should preserve both launchers: Rust devtool for the active native editor/runtime viewport work, Python viewer for parity-ledger inspection until Rust reaches true parity.
- Updated the architecture contract so agents do not remove the Python ledger launcher as stale clutter. The Python surface may inspect checked-in artifacts, but CLI/shared Rust artifacts remain the authority for extraction, import/export, validation, and engine behavior.

## 2026-06-11 ISO-backed competitive stage extraction

- Added the Rust CLI path `mole stage extract-iso --iso <path> --competitive --write`, which reads the local GameCube ISO FST directly and does not depend on Python for stage DAT acquisition.
- The command mirrors raw stage DATs into ignored local raw inputs, then feeds each DAT through the same `mole stage extract` MapCollData parser so stage blobs use one source-of-truth pipeline.
- Extracted the current baseline competitive stage set from the local Melee 1.02 ISO:
  - Battlefield: `GrNBa.dat`
  - Final Destination: `GrNLa.dat`
  - Yoshi's Story: `GrSt.dat`
  - Fountain of Dreams: `GrIz.dat`
  - Dream Land 64: `GrOp.dat`
  - Pokemon Stadium: `GrPs.dat` plus sidecar transformation DATs `GrPs1.dat` through `GrPs4.dat`
- Compact reviewable stage blobs now exist under `resources/melee/extracted/stages/*_stage.json`. Battlefield also compiles into the game-owned engine blob at `crates/mole_core/src/generated/stages.rs`; this committed Rust blob is what lets a fresh checkout keep building after the ISO/decomp/raw DATs are removed.
- Important boundary: extracted stage blobs can contain arbitrary collision lines and many derived surfaces. `mole_core::StageProfile` is still a fixed launch-test shape with one main floor and three soft platforms, so the next engine slice should introduce a variable extracted-stage collision model before trying to run non-Battlefield stages in rollback/runtime.

## 2026-06-11 Battlefield source-float core stage boundary

- Added `crates/mole_core/src/generated/stages.rs`, generated by `mole stage extract --stage battlefield --write`, as the engine-facing Battlefield stage source object. It preserves the raw DAT collision vertices as `f32`, the decomp `grGroundParam` scale as `f32`, and the MapCollData line indices, adjacency ids, flags, passability, and collision kinds.
- `mole_core::MeleeStageProfile::battlefield()` now returns the generated Battlefield blob instead of a hand-authored duplicate in `stage.rs`.
- `StageProfile::battlefield()` is now a compatibility projection from `MeleeStageProfile::battlefield()`. It exists for older ground-contact/render code that still expects one main floor and three soft platforms in milli-units.
- `World::for_two_players()` and `World::for_slippi_battlefield_singles_match_start()` now default to `StageProfile::battlefield()` instead of the old `battlefield_test` placeholder, so the SillyBee/replay launch-test path starts from the extracted Battlefield source boundary.
- `generated write-stage-asset` now prefers raw DAT extraction when `resources/melee/raw/GrNBa.dat` exists, preventing the legacy generated command from overwriting the extracted Battlefield blob with the old baked placeholder.
- `stage inspect --stage battlefield` now checks the generated engine blob as part of parity readiness, so the CLI can verify that the extraction result is actually implemented into the game and not only parked in research/middleware.
- Verification for this slice included `cargo test -p mole_core`, `cargo test -p mole_runtime`, focused `mole_cli` stage generator/extractor tests, and the Python resource/value-sheet tests.
