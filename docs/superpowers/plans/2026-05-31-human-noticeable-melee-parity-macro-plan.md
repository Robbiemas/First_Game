# Human-Noticeable Melee Parity Macro Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reach human-noticeable Captain Falcon movement parity on Battlefield before expanding into full combat, full roster, or global Melee completeness.

**Architecture:** Rust remains authoritative for deterministic 60 Hz simulation, rollback input snapshots, action states, physics, collision, ECB, checksums, replay, and rollback. Pygame remains only a legacy reference/harness if needed; SDL3 is the native runtime shell. UCF stays outside the engine in `mole_input`/WUP adapter preprocessing, while the Rust core consumes vanilla Melee-shaped input snapshots and facts.

**Tech Stack:** Rust workspace (`mole_core`, `mole_input`, `mole_runtime`, `mole_replay`, `mole_rollback`, `mole_cli`), generated Melee DAT resources under `resources/melee/extracted`, Python dev tools under `tools`, JSON graph/value ledgers under `docs/state_graphs`, local decomp reference under `D:\Mole Game\.research\doldecomp-melee`.

---

## Compaction Anchor

If context is compacted, resume from this anchor before touching code:

1. The target is not "all of Melee first." The target is **human-noticeable Falcon-like movement parity on Battlefield**.
2. Do not guess mechanics. Compare against local decomp source and extracted DAT values before changing behavior.
3. The important chain is: controller sample -> adapter/UCF preprocessing -> vanilla input facts/timers -> action-state priority -> state entry/exit -> velocity fields -> physics update order -> collision/ECB -> render/debug output.
4. UCF belongs to `mole_input`/WUP adapter. The core engine models vanilla Melee.
5. ECB import/mapping for the current Falcon movement sandbox is sufficiently covered: `70` mapped motion states, `0` missing sampled ECB mappings, `0` unmapped derived states. Future ECB work should focus on correct use at the right source-equivalent moment, not importing unrelated global data.
6. The next highest-value parity work is source-shaped state and physics update order, especially Dash/Turn/Run, `gr_vel`/`self_vel`, IASA timing, animation callbacks, and replay oracle divergence.
7. Keep dev tools congruent. Every behavior slice updates tests, graph docs, value sheets/reports if values change, and parity snapshot expectations.
8. Current known graph gap at the time this plan was written: `RunDirect -> Run` is still the only missing graph item. Treat it as source-audit work, not an ECB import issue.

## Human-Noticeable Scope

This plan defines the first playable parity target as:

- Character: Captain Falcon values applied to the Dolphin Mole test character.
- Stage: Battlefield-sized test stage with Melee unit scaling.
- Runtime: SDL3 native runtime launched by `execs\Run SDL3 Runtime.cmd`.
- Input: native WUP/GameCube path first, UCF default on, vanilla toggle available for diagnostics.
- Movement slice: Wait, WalkSlow/Middle/Fast, Turn, Dash, Run, TurnRun, RunDirect identity, RunBrake, Squat/SquatWait/SquatRv, KneeBend, JumpF/B, JumpAerialF/B, Fall/FallF/FallB, FallAerial/F/B, EscapeAir, FallSpecial/F/B, Landing, LandingFallSpecial, platform pass/drop, shield movement, roll, spotdodge, and ledge grab once implemented.
- Replay oracle: Slippi replay comparison should advance from spawn/entry through the first movement-only sequence and eventually to the staged ledge-grab moment around the 30-second mark.

Not required for this first target:

- Every Melee character's ECB and attribute data.
- Full combat hitboxes, hurtboxes, knockback, hitlag, SDI/ASDI, stale move handling, shield damage, throws, item states, damage states, tech states, or respawn platform polish.
- A fully generalized character editor, although value sheets and graph ledgers should keep moving toward that future.

## Source And Tool Anchors

- Common decomp movement source: `D:\Mole Game\.research\doldecomp-melee\src\melee\ft\chara\ftCommon`
- Fighter update order: `D:\Mole Game\.research\doldecomp-melee\src\melee\ft\fighter.c`
- HSD animation/collision helpers: `D:\Mole Game\.research\doldecomp-melee\src\melee\lb`, `D:\Mole Game\.research\doldecomp-melee\src\melee\mp`
- Core engine: `crates/mole_core/src/state.rs`, `crates/mole_core/src/sim.rs`, `crates/mole_core/src/input.rs`, `crates/mole_core/src/stage.rs`
- Input adapter: `crates/mole_input/src/lib.rs`, `crates/mole_runtime/src/wup_input.rs`
- Runtime/debug visuals: `crates/mole_runtime/src/main.rs`, `crates/mole_runtime/src/readout.rs`, `crates/mole_runtime/src/assets.rs`
- Generated ECB: `crates/mole_core/src/generated/falcon_ecb.rs`
- Extracted values: `resources/melee/extracted/*.json`
- Graphs and ledgers: `docs/state_graphs/*.json`, `docs/state_graphs/value_sheets/*.json`, `docs/state_graphs/parity_reports/*.json`
- Dev tool: `tools/state_graph_viewer.py`
- Extractors/generators: `tools/extract_melee_resources.py`, `tools/generate_falcon_ecb_rust.py`, `tools/generate_value_sheets.py`, `tools/export_parity_diff_report.py`
- CLI coordination: `cargo run -p mole_cli -- help --json`, `cargo run -p mole_cli -- parity snapshot --json`, `.\target\debug\mole.exe request list --json`

## Phase 0: Always-On Work Rules

**Purpose:** Keep the project from drifting into local approximations while still moving quickly.

- [ ] Before changing movement behavior, identify the decomp function, extracted field, or replay oracle fact that justifies the change.
- [ ] Add or update a Rust test before production behavior changes unless the task is documentation-only or generated artifact refresh.
- [ ] Keep tests focused on observable state, velocity, collision, checksum, replay, or debug-output behavior.
- [ ] Do not add custom named gameplay states for emergent mechanics such as wavedash, moonwalk, shield drop, or AXE drop.
- [ ] Do not put UCF behavior into `mole_core`; keep raw/native/UCF diagnostics in adapter/runtime layers.
- [ ] Update graph/value/dev-tool artifacts in the same slice as behavior changes.
- [ ] Run the verification gate listed at the end of this plan before claiming a slice is complete.

## Phase 1: Parity Ledger As Source Navigation

**Purpose:** Turn the dev tool into the persistent map of what exists, what is source-backed, and what remains partial.

### Task 1.1: Keep graph status honest

- [ ] For every movement state touched, update `docs/state_graphs/mole_current_graph.json` with source refs, Rust refs, value refs, and known gaps.
- [ ] Use `aligned` only when the transition/value is source-backed and covered by tests.
- [ ] Use `partial` when state identity exists but update order, callbacks, animation metadata, or edge cases remain.
- [ ] Use `missing` only when the state/edge is absent or cannot be reached through a source-backed path.
- [ ] Run `.venv\Scripts\python.exe tools\state_graph_viewer.py --check`.

### Task 1.2: Make missing graph work queryable

- [ ] Periodically run `.\target\debug\mole.exe request list --json`.
- [ ] If the side-agent completes `missing-graph-entries`, use the new command to inspect missing graph entries before ad-hoc JSON parsing.
- [ ] Until that command exists, use `cargo run -p mole_cli -- parity snapshot --json` and `tools/state_graph_viewer.py --check`.

## Phase 2: Replay Oracle Pipeline

**Purpose:** Reduce dependence on human feel testing by replaying real Slippi inputs against the Rust core.

### Task 2.1: Establish replay comparison checkpoints

- [ ] Keep `tools/slippi_replay_to_inputs.cjs` exporting Melee-shaped pad snapshots, UCF metadata, and adapter-owned dashback amendments.
- [ ] Use the replay in `Replays` as the first movement-only oracle.
- [ ] Divide the replay into checkpoints: match start/entry, grounded dash sequence, jump/air movement, platform pass/drop, and the ledge-grab setup near 30 seconds.
- [ ] For each checkpoint, write or update Rust/runtime tests that report first divergent frame, player, state, position, velocity, active ECB, and input facts.
- [ ] Do not seed arbitrary state if the lower-level spawn/entry/platform logic can be implemented from source instead.

### Task 2.2: Keep replay diagnostics visible in the dev tool

- [ ] Display replay comparison summary in the dev tool without replacing the existing graph and value ledger.
- [ ] Include first mismatch frame, expected Slippi state, Rust state, position delta, velocity delta, and active ECB sample name if available.
- [ ] Keep generated reports under `docs/state_graphs/parity_reports` or `debug/slippi` depending on whether they are durable docs or local diagnostics.

## Phase 3: Input Boundary Parity

**Purpose:** Make sure controller interpretation is not the reason human muscle memory fails.

### Task 3.1: Preserve adapter/core separation

- [ ] Verify raw WUP bytes, console-origin adjusted sample, HSD-clamped sample, optional UCF sample, and final core `PlayerInput` are all visible in logs.
- [ ] Keep `execs\Run SDL3 Runtime.cmd` launching UCF-on native play.
- [ ] Keep `execs\Run SDL3 Runtime Vanilla No UCF.cmd` available for pre-UCF diagnostics.
- [ ] Confirm `mole_input` owns UCF cardinal, shield-drop, and dashback preprocessing.
- [ ] Confirm `mole_core` only consumes vanilla snapshots/facts plus deterministic adapter-owned amendment bits already present in input.

### Task 3.2: Compare human WUP logs to replay-derived inputs

- [ ] Add focused log snippets for dash dance, moonwalk attempt, wavedash, shield drop attempt, and platform pass.
- [ ] Compare WUP logs against equivalent Slippi/TAS payload frames at the level of core input facts and timers.
- [ ] If WUP differs before core facts, fix adapter/input preprocessing.
- [ ] If WUP facts match but movement differs, fix core state/physics order.

## Phase 4: Grounded Locomotion And Moonwalk Feel

**Purpose:** Make Falcon grounded control feel right before adding broader systems.

### Task 4.1: Audit source update order for Wait, Walk, Turn, Dash, Run, RunBrake, TurnRun, RunDirect

- [ ] For each state, document the decomp order: Anim, IASA/Input, Phys, Coll, and any callback-specific field writes.
- [ ] Track exact use of `gr_vel`, `self_vel`, `xE4_ground_accel_1`, `xE8_ground_accel_2`, motion vars, facing, and input timers.
- [ ] Add tests proving Dash entry, Dash IASA, Dash live-stick acceleration, Turn facing flip, TurnRun old-facing behavior, RunBrake friction, and Dash completion match source-shaped ordering.
- [ ] Keep moonwalk as an emergent Dash/Turn/Walk/velocity outcome.

### Task 4.2: Use moonwalk as a diagnostic, not a custom mechanic

- [ ] Keep the bottom-gate payload test: `(+127,0) -> (-101,-45) -> (-101,-45) -> (-128,0)`.
- [ ] Add a walk-carry moonwalk test: full-speed walk right, one-frame dash left, bottom-gate roll, full rightward influence without center deadzone turnaround.
- [ ] Add chained moonwalk/foxtrot relay tests before initial Dash animation resolves.
- [ ] Compare resulting distance and state sequence to source/replay observations rather than tuning constants.

### Task 4.3: Resolve `RunDirect -> Run`

- [ ] Search local decomp for all `ftCo_MS_RunDirect` references before changing behavior.
- [ ] Document whether `RunDirect` is reachable from common action flow, character-specific callbacks, or replay-only action-state identity.
- [ ] If reachable, implement the source entry and `ftCo_RunDirect_IASA` behavior with tests.
- [ ] If not reachable in the current movement sandbox, keep it as explicit state identity with a documented graph gap.

## Phase 5: Airborne Movement, Wavedash, And Landing

**Purpose:** Make jump, air drift, air dodge, wavedash, and landing feel source-shaped.

### Task 5.1: Audit jump and aerial velocity ownership

- [ ] Verify KneeBend completion, short-hop release, JumpF/B entry, JumpAerialF/B entry, and Fall/FallAerial transitions against source.
- [ ] Track where source uses `init_h_vel`, `self_vel`, `gr_vel`, `x74_anim_vel`, and ground-to-air momentum multiplier.
- [ ] Add tests for jumping out of walk, dash, slide, and moonwalk carry.
- [ ] Confirm air drift uses extracted Falcon `air_drift_stick_mul`, `aerial_drift_base`, `air_drift_max`, and `aerial_friction`.

### Task 5.2: Lock EscapeAir and LandingFallSpecial as wavedash ingredients

- [ ] Confirm EscapeAir force, decay, IASA timer, animation duration, deadzone, and landing lag are all PlCo sourced.
- [ ] Confirm EscapeAir does not use ordinary gravity during action frames.
- [ ] Confirm animated ECB bottom probes are used for previous/current landing contact.
- [ ] Confirm LandingFallSpecial preserves horizontal self velocity and applies source-shaped ground traction.
- [ ] Use Slippi/replay checkpoints and human testing only after these tests pass.

### Task 5.3: Finish ftAnim fall submotion metadata when it becomes useful

- [ ] Keep current active directional ECB pose selection for Fall/FallAerial/FallSpecial.
- [ ] Defer full ftAnim blend accumulator implementation until visible animation/collision timing depends on it.
- [ ] Do not treat full ftAnim recreation as a blocker for human-noticeable empty movement unless a replay or test proves it affects collision or state timing.

## Phase 6: Platforms, Collision, And Ledges

**Purpose:** Make Battlefield movement interactions source-shaped enough for replay progression and human testing.

### Task 6.1: Finish platform pass/drop behavior from the bottom up

- [ ] Audit source platform callbacks for ordinary Fall, FallSpecial, Pass, Shield/Guard, Squat, and aerial actions.
- [ ] Implement pass/drop as source states and collision callbacks, not one-off platform hacks.
- [ ] Test platform landing acceptance/rejection using active ECB bottom probes and stick/timer facts.
- [ ] Keep soft-platform and main-floor behavior separate in stage data.

### Task 6.2: Implement ledge grab as the next replay milestone

- [ ] Audit Melee ledge/cliff source before adding any ledge behavior.
- [ ] Add Battlefield ledge anchors to stage profile using source unit scale.
- [ ] Add ledge grab detection from airborne collision/ECB state, not from sprite position.
- [ ] Use the replay's left/right ledge grab near 30 seconds as the first acceptance checkpoint.

## Phase 7: Defensive Movement

**Purpose:** Complete movement-relevant shield and dodge behavior before combat boxes.

### Task 7.1: Shield movement and platform interaction

- [ ] Keep bubble shield visual as runtime rendering only until real shield model is needed.
- [ ] Verify GuardOn, Guard, GuardOff, GuardReflect, GuardSetOff identity, shield jump, shield drop, spotdodge, roll, and pass priority against source.
- [ ] Keep AXE drop and shield drop as emergent input/collision outcomes, with UCF preprocessing only in adapter.
- [ ] Add tests for vanilla shield drop, UCF-assisted shield drop, spotdodge priority, roll priority, shield jump, and platform pass.

### Task 7.2: Roll and spotdodge root motion

- [ ] Audit EscapeF, EscapeB, and EscapeN source for duration, collision, invulnerability timing hooks, and movement/root motion.
- [ ] Implement root-motion-backed travel only from source data or source logic.
- [ ] Leave invulnerability visualization/combat effects for the combat phase if they do not affect movement feel yet.

## Phase 8: Combat Readiness Boundary

**Purpose:** Enter hitboxes and hurtboxes only after movement is trustworthy.

### Task 8.1: Define combat prerequisites

- [ ] Confirm movement replay checkpoints through ledge grab are stable.
- [ ] Confirm core state transitions and physics update order are documented in graph/dev tool.
- [ ] Confirm value ledgers report zero actionable diffs for currently imported global and Falcon values.
- [ ] Confirm runtime debug overlay exposes states, inputs, velocities, active ECB, and checksums.

### Task 8.2: Start combat in source order

- [ ] Add hitbox/hurtbox ownership and frame data before damage states.
- [ ] Add hitlag before knockback behavior.
- [ ] Add knockback and tumble before tech/knockdown.
- [ ] Add shield damage, shield hit states, SDI/ASDI, and stale move handling after the base hit interaction is deterministic.

## Phase 9: Editor And Character-Creation Future

**Purpose:** Let today's parity tooling become tomorrow's character editor without letting editor work distract from movement parity.

### Task 9.1: Keep data boundaries editor-friendly

- [ ] Keep global common values separate from character profile values.
- [ ] Keep generated extracted Melee snapshots separate from editable future character data.
- [ ] Keep graph/value reports read-only until the editor explicitly owns mutation.
- [ ] Keep runtime debug overlays and dev tool tables aligned with Rust source data.

### Task 9.2: Delay editor mutation until the parity base is stable

- [ ] Do not build value-edit UI while core movement parity is still drifting.
- [ ] Do not let editor convenience change engine architecture.
- [ ] When ready, build the editor as a data authoring layer over the same Rust profile/common-data seams already proven by tests.

## Verification Gate

Run these before claiming any phase checkpoint is complete:

```powershell
cargo fmt
cargo test --workspace
.venv\Scripts\python.exe -m pytest tests\test_value_sheets.py tests\test_state_graph_viewer.py tests\test_launch_inputs.py tests\test_parity_diff_report.py tests\test_generate_falcon_ecb_rust.py tests\test_extract_melee_resources.py tests\test_slippi_replay_tools.py -q
.venv\Scripts\python.exe tools\state_graph_viewer.py --check
cargo run -p mole_cli -- parity --json
cargo run -p mole_cli -- parity snapshot --json
git diff --check
```

Expected current baseline at plan creation:

- Rust workspace tests pass.
- Selected Python/dev-tool suite passes.
- Graph has no mismatches.
- Graph has one known missing item: `RunDirect -> Run`.
- Value ledger reports `102/102` matching rows: `63` global and `39` test-character.
- Falcon ECB coverage reports `70` mapped motion states, `0` missing sampled mappings, and no unmapped derived states.

## Human Test Gate

Ask for human testing only after automated checks pass and the runtime is launchable. Suggested tests, in order:

1. Dash dance both directions with UCF on.
2. Dash dance both directions with UCF off.
3. Full-speed walk into moonwalk attempt.
4. Chained moonwalk/foxtrot relay attempt.
5. Short-hop, full-hop, double-jump, air drift, fast fall.
6. Wavedash both directions from short-hop air dodge.
7. Platform pass/drop from squat, fall, and shield.
8. Roll, spotdodge, shield jump, shield drop.
9. Replay oracle visual comparison through the current checkpoint.

Treat human feedback as symptom data. Use logs, replay, and source audit to find the root cause before changing mechanics.

## Commit And Integration Policy

- Commit only when the user asks or when a branch-completion workflow is explicitly approved.
- Keep unrelated dirty files intact.
- Prefer small commits by phase when committing is requested.
- Do not merge to `main` until the user has tested the SDL runtime and the verification gate is green.
