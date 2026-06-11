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
- The editor direction is now a single Rust geometry-editing core for joints, capsules, and ECB/body volumes, with save/export staying on the current JSON artifact shape first.
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
- `battlefield_stage_values.json` is intentionally sourced from the current Rust `StageProfile::battlefield_test()` data and marked as pending the generalized decomp stage extraction pipeline.
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

- Added an in-place drag path for a selected joint handle so the editor mutates only the targeted joint instead of rebuilding the frame.
- The joint drag helper updates the existing world matrix translation components directly and keeps the Z axis intact for now.
- The editor marks itself dirty only when a real mutation happens, which keeps the change model cheap and easy to export later.
- Current focus remains on simple, fast primitives that can be layered into an exportable animation-set backend without introducing lag.

## 2026-06-10 move keyframes save round-trip slice

- Added a Rust save path that round-trips the existing `AttackAirN.json` shape without introducing a new animation format.
- `MoveKeyframesSurface::load_from` now supports arbitrary JSON paths so save/export can be tested against temp files.
- The editor clears dirty state only after a successful write, which keeps the save behavior predictable and cheap.
- This preserves the current artifact shape as the export contract, which is important for future Unity / Godot backends.

## 2026-06-10 move keyframes handle interaction slice

- The `Move Keyframes` preview now draws draggable handle markers for the selected frame.
- Clicking and dragging a joint handle mutates the matching joint in place using the current preview scale, with no reparsing of the frame JSON.
- The active handle state is kept tiny: one active handle plus accumulated screen delta, which keeps the interaction path responsive.
- The save button is now exposed in the Rust UI and writes back to the current artifact path when the editor is dirty.

## 2026-06-10 move keyframes collision-pill drag checkpoint

- The shared drag helper also works for hurtbox endpoints, which means the same low-overhead editor path covers the collision-pill style geometry too.
- The editor still mutates in place and preserves the 3D data model in Rust, so the UI remains just a thin view layer over the core data.

## 2026-06-10 move keyframes verification pass

- Ran the full focused move-keyframes test filter: `cargo test -p mole_devtool move_keyframes_ -- --nocapture`.
- Result: all move-keyframes editor tests passed, including skeleton loading, joint drag, hurtbox drag, and save round-trip.
- This slice is now stable enough to build the next editor primitives on top of without revisiting the current tree/drag/save baseline.
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
- The lower collapsible sheet stays as the exposed value surface for the full frame data, which keeps hitbox and hurtbox fields available for future direct editing without forcing the preview layout to carry every detail inline.

## 2026-06-10 move keyframes debug stage and inspector

- The move-keyframes preview now uses a dedicated flat debug stage instead of Battlefield, with only one visible ground surface in the scene.
- The preview viewport now fit-factors the rendered runtime geometry so the character, collision shapes, and imported figatree joints stay centered and zoomed appropriately.
- The viewport now overlays the imported joint hierarchy so all nodes and joints are visible in the 2D projection, not just the collision pills.
- A visible selected-frame inspector was added under the timeline strip so keyframe metadata and hitbox/hurtbox/body-volume values remain exposed outside the click-drag handles.
- Float-based engine values should stay float-based in the runtime/editor path; only frame indices and other discrete counters stay integral.

## 2026-06-10 move keyframes right-facing overlay and direct inspector

- The move-keyframes overlay now uses the right-facing flattened basis that the runtime uses: screen x maps to source z and screen y maps to source y.
- Joint dragging updates the flattened axes instead of the depth axis, so the on-screen rig stays aligned with the hurtbox pills.
- The frame strip compresses horizontally to fit the window instead of turning into a scroll wheel.
- The right-hand panel now exposes direct editable fields for the first hitbox, hurtbox, and body volume instead of a JSON dump.
- The Rust devtool continues to repurpose existing editor/runtime code rather than rewriting the wheel.
- The joint overlay now uses the selected frame's sampled pose joints directly, so the on-screen rig follows the same frame basis as the editor handles.
- The viewport height is now tighter so the selection/inspector region can stay visible in the same window without clipping.

## 2026-06-11 functional handoff cleanup

- The Rust devtool now has one shared theme palette module for workbench identity colors and status colors; table rendering consumes the shared palette instead of carrying duplicate row colors.
- The devtool defaults to the light Python-viewer-style workbench while retaining a dark toggle.
- The move-keyframes viewport keeps `RenderFrame -> RenderScene` as the visual authority and no longer draws the imported pose-tree rig as a default overlay.
- Editable handles remain as the annotation layer over the runtime scene. Selecting a handle exposes focused values in the right inspector, and dragging/editing the selected handle updates the projected artifact `x/y` fields.
- The old `Open State Graphs.cmd` entrypoint was replaced by `Open Dev Tool.cmd`; runtime launcher contracts now point at the renamed Rust devtool launcher.
- Verification for this handoff:
  - `cargo fmt --check`
  - `cargo test -p mole_devtool`
  - `cargo test -p mole_cli`
  - `cargo test -p mole_runtime`
  - `cargo test -p mole_core`
