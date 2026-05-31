# Grounded Locomotion Difference Sheet

Date: 2026-05-31

Scope: Captain Falcon, Battlefield, vanilla grounded normal movement. This covers
Wait, WalkSlow, WalkMiddle, WalkFast, Turn, Dash, Run, RunBrake, TurnRun, and
RunDirect, plus the input, value, physics, collision, ECB, and runtime trace
factors that feed those states.

Non-goals: full combat, full roster parity, global Melee completeness, UCF inside
the Rust core, or adding custom gameplay states for emergent mechanics.

## Evidence Used

- `cargo run -p mole_cli -- agent brief --format markdown`
- `cargo run -p mole_cli -- parity --json`
- `cargo run -p mole_cli -- generated check --format markdown`
- `cargo run -p mole_cli -- graph inspect <state-or-edge> --format markdown`
  for Wait, WalkSlow, Turn, Dash, Run, RunDirect, RunBrake, TurnRun,
  `Dash -> Run`, `Run -> TurnRun`, `TurnRun -> Run`, and `Run -> RunBrake`
- Local decomp:
  - `.research/doldecomp-melee/src/melee/ft/fighter.c`
  - `.research/doldecomp-melee/src/melee/ft/ftmotionstates.c`
  - `.research/doldecomp-melee/src/melee/ft/ftcommon.c`
  - `.research/doldecomp-melee/src/melee/ft/ft_084E.c`
  - `.research/doldecomp-melee/src/melee/ft/ft_081B.c`
  - `.research/doldecomp-melee/src/melee/ft/ft_0892.c`
  - `.research/doldecomp-melee/src/melee/ft/ftwalkcommon.c`
  - `.research/doldecomp-melee/src/melee/ft/inlines.h`
  - `.research/doldecomp-melee/src/melee/ft/chara/ftCommon/ftCo_Wait.c`
  - `.research/doldecomp-melee/src/melee/ft/chara/ftCommon/ftCo_Walk.c`
  - `.research/doldecomp-melee/src/melee/ft/chara/ftCommon/ftCo_Turn.c`
  - `.research/doldecomp-melee/src/melee/ft/chara/ftCommon/ftCo_Dash.c`
  - `.research/doldecomp-melee/src/melee/ft/chara/ftCommon/ftCo_Run.c`
  - `.research/doldecomp-melee/src/melee/ft/chara/ftCommon/ftCo_RunBrake.c`
  - `.research/doldecomp-melee/src/melee/ft/chara/ftCommon/ftCo_TurnRun.c`
  - `.research/doldecomp-melee/src/melee/ft/chara/ftCommon/ftCo_RunDirect.c`
- Rust:
  - `crates/mole_core/src/sim.rs`
  - `crates/mole_core/src/input.rs`
  - `crates/mole_core/src/state.rs`
  - `crates/mole_core/src/common_data.rs`
  - `crates/mole_runtime/src/readout.rs`
  - `crates/mole_runtime/src/lib.rs`

Dev-tool baseline during this audit:

- Value parity: 102/102 matching rows, 63 global and 39 Falcon/test-character.
- Falcon ECB coverage: 70 mapped motion states, 0 missing sampled mappings,
  0 unmapped derived states.
- State graph: 52 nodes, 71 edges, 21 aligned, 3 intentional, 99 partial.
- `mole generated check` reports dirty generated outputs and one stale ECB group.
  This is a workspace hygiene note, not a new behavior finding from this sheet.

## Source Update Order

This matters because several movement decisions are not just input predicates.
They depend on animation callbacks, command variables, and motion variables that
are updated before IASA and physics.

Melee registers fighter procs in `fighter.c:906-920`:

| Priority | Proc | Grounded movement relevance |
| --- | --- | --- |
| 1 | `Fighter_8006A360` | advances animation and calls `anim_cb` at `fighter.c:1705-1706` |
| 3 | `Fighter_Spaghetti_8006AD10` | updates input/timers and calls `input_cb` at `fighter.c:2120-2121` |
| 4 | `Fighter_procUpdate` | calls `phys_cb` at `fighter.c:2160-2161` |
| 6 | `Fighter_procMap` | calls `coll_cb` at `fighter.c:2476-2477` |

The state table in `ftmotionstates.c` wires each motion state to its
Anim/IASA/Phys/Coll callbacks. The normal movement slice around Turn, TurnRun,
Dash, Run, RunDirect, and RunBrake is visible at `ftmotionstates.c:330-399`.

Rust currently runs a single per-player state branch in `sim.rs:89-320`, then
commits position and ground velocity in `sim.rs:692-699`, then performs a
simplified floor-support collision check. This keeps deterministic rollback
simple, but it collapses source `Anim -> IASA -> Phys -> Coll` separation for
several states. That is the main architectural difference under the remaining
human-noticeable grounded movement gaps.

## Shared Physics Factors

| Factor | Melee source | Rust current | Status |
| --- | --- | --- | --- |
| Input thresholds | PlCo fields such as `x24`, `x34`, `x38`, `x3C`, `x40`, `x44`, `x48`, `x4C`, `x54`, `x58`, `x5C`, `x60`, `x430` | Extracted through `MeleeCommonData`; value ledger is 102/102 | Mostly aligned |
| Dash/run accel and target | `getAccelAndTarget` in `inlines.h:130-137` | `dash_run_accel_and_target` in `sim.rs:2050` | Aligned in shape |
| Ground accel toward target | `ftCommon_8007C98C` in `ftcommon.c:71-105` | `apply_ground_accel_toward_target` in `sim.rs:2174` | Partial: source max clamp inside some branches is simplified |
| General ground friction | `ft_80084F3C` in `ft_084E.c:41-52` with high-speed multiplier over walk max | `apply_ground_traction` in `sim.rs:2009` | Mostly aligned for flat Battlefield |
| Run friction | run/turn/brake use `gr_friction * x60` | `run_ground_friction` in `sim.rs:2027` | Aligned in value, simplified surface multiplier |
| Ground movement projection | `ftCommon_ApplyGroundMovement` in `ftcommon.c:143-160` projects through floor normal into anim/self velocity | Rust uses flat-stage integer `ground_velocity_x`, `ground_accel_x`, `ground_accel_x2` | Partial: Battlefield main floor ok, slopes/normal projection not complete |
| Ground collision | `ft_80084280` and `ft_800844EC` in `ft_081B.c:1069-1142` handle ledges, nudges, fall, edge behavior | `has_floor_support` plus fall transition after position commit | Partial, acceptable on center Battlefield floor only |
| Motion command vars | source `cmd_vars[0]`, `cmd_vars[1]` gate Dash, RunBrake, TurnRun behavior | mostly absent from Rust locomotion state | Gap |
| Animation-frame gates | source `cur_anim_frame`, `ftAnim_IsFramesRemaining`, anim rate, and motion vars gate exits | Rust uses `motion_frame`, fixed profile frame counts, and immediate branch checks | Gap for TurnRun/RunBrake, risk for Dash/Walk |

## State Differences

### Wait

Source:
- `ftCo_Wait_Anim` can enter DownSpot or loop wait animation.
- `ftCo_Wait_IASA` priority is in `ftCo_Wait.c:43-65`.
- `ftCo_Wait_Phys` uses `ft_80084F3C`, then `ftColl_8007AEE0`.
- `ftCo_Wait_Coll` uses `ft_80084280`.

Rust:
- `apply_wait_state_inputs` in `sim.rs:780` handles grounded actions, shield,
  jump, dash, turn, crouch, walk, and ground traction.
- Graph marks Wait aligned.

Differences:
- `ftColl_8007AEE0` has no direct Rust equivalent in Wait physics.
- Some helper branches in the source priority list are still represented by
  broader Rust action checks, not source helper identities.
- DownSpot/character-specific wait animation behavior is outside the current
  grounded Falcon movement sandbox.

Impact:
- Low for Falcon movement on Battlefield center.
- Medium later if ledge/edge, wait animation, or character-specific idle logic
  becomes part of parity.

### WalkSlow / WalkMiddle / WalkFast

Source:
- `ftCo_Walk_CheckInput` enters walk when `lstick.x * facing >= x24`.
- `ftCo_Walk_Enter` passes animation-frame/rate values into
  `ftWalkCommon_800DFCA4`.
- `ftWalkCommon_GetWalkType` chooses Slow/Middle/Fast from current `gr_vel`
  against `x28 * walk_max_vel` and `x2C * walk_max_vel`.
- `ftWalkCommon_800DFDDC` sets animation rate from `gr_vel` or stored
  `mv.co.walk.x0`.
- `ftWalkCommon_800DFEC8` changes walk bucket while preserving animation phase.
- `ftWalkCommon_800E0060` applies walk accel, taper `x30`, stores
  `mv.co.walk.x0 = target_vel * x440`, and applies ground movement.

Rust:
- `walk_motion_state` in `sim.rs:1038` chooses walk bucket from
  `ground_velocity_x`.
- `apply_walk_velocity` in `sim.rs:1900` implements source-shaped initial
  velocity, accel, target, and taper.
- Graph marks WalkSlow/Middle/Fast aligned.

Differences:
- Rust does not model walk animation phase remap from `ftWalkCommon_800DFEC8`.
- Rust does not carry `mv.co.walk.x0` as a separate animation-rate source.
- Metal/scale/item held movement multipliers in `ftCo_Walk_Enter` are absent.
- Input debug buckets still have provisional `walk_slow_x`, `walk_middle_x`,
  and `walk_fast_x` values. State selection itself is velocity-based.

Impact:
- Low for raw flat-ground displacement.
- Medium for visible animation feel and walk bucket transitions.

### Turn

Source:
- `ftCo_Turn_Anim_Inner` flips facing after `frames_to_turn`.
- `ftCo_Turn_IASA` temporarily flips facing before and after early action checks,
  latches A/B through `x1C`, arms dash-out via `fn_800C9C2C`, and enters Dash
  on `just_turned` if the stick is still held past `x3C`.
- `ftCo_Turn_Phys` uses `ft_80084F3C`.
- `ftCo_Turn_Coll` uses `ft_80083F88`.

Rust:
- Turn lives in `sim.rs:273-320`, with `advance_turn_anim`,
  `turn_effective_input_facts`, `arm_turn_dash_after_if_fresh`, and same-frame
  ground traction on entry.
- UCF dashback amendment is adapter/runtime-owned, not core-owned.
- Graph marks Turn partial.

Differences:
- Rust approximates the temporary-facing IASA behavior rather than carrying the
  exact source facing-flip sequence through all source helper calls.
- Rust has button latching, but not the exact `x1C` helper behavior for every
  action branch.
- Turn collision remains simplified relative to `ft_80083F88`.

Impact:
- Medium for dashback and turnaround input edge cases.
- Lower for normal standing turn on flat floor.

### Dash

Source:
- `ftCo_Dash_CheckInput` uses `abs(lstick.x) >= x3C` and
  `x670_timer_lstick_tilt_x < x40`.
- `ftCo_Dash_Enter` sets `cmd_vars[0] = 0`, resets x tap timer, computes
  `mv.co.dash.x0`, calls `ftCommon_800804A0`, and stores tap-start flag `x4`.
- `ftCo_Dash_Anim` falls back through `ft_8008A2BC` on animation completion.
- `ftCo_Dash_IASA` uses early/mid/late windows `x44`, `x48`, `x4C`, animation
  `cmd_vars[0]`, `fn_800CA5F0` for Run, and x54 velocity decay fallthrough.
- `ftCo_Dash_Phys` skips live accel on the staged dash-entry frame, then uses
  dash/run accel and target with run friction.
- `ftCo_Dash_Coll` uses `ft_800844EC`.

Rust:
- Dash lives in `sim.rs:136-178`.
- Entry stages the dash delta as `ground_accel_x2`, matching the source shape
  where dash-entry velocity commits after same-frame translation.
- `apply_dash_velocity` and `dash_iasa_decayed_ground_velocity` cover live dash
  accel and x54 decay.
- Graph marks Dash and `Dash -> Run` aligned.

Differences:
- Rust does not model `cmd_vars[0]` as an animation-script output. Run exit is
  modeled by `profile.dash_frames` and source windows rather than the actual
  animation command variable.
- Source `Dash_Anim` can move to Wait before IASA/Phys on animation completion;
  Rust exits Dash near the end of its Dash branch after physics staging.
- Dash collision/edge handling is simplified.

Impact:
- Low to medium. Existing tests cover a lot of dash and moonwalk-like payload,
  but exact animation-command timing can still show up as one-frame feel errors.

### Run

Source:
- `fn_800CA5F0`: Dash-to-Run if same-facing stick reaches `x58`.
- `fn_800CA644`: TurnRun-to-Run if same-facing stick reaches `x58`, entering
  Run with no-interrupt window `x430`.
- `fn_800CA698`: RunDirect-to-Run if same-facing stick reaches `x58`.
- `ftCo_Run_Anim` sets animation rate from `gr_vel` or `mv.co.run.x4` and
  decrements `mv.co.run.x0`.
- `ftCo_Run_IASA` checks action/shield/jump paths, then:
  - if `mv.co.run.x0 <= 0`, `fn_800C9D40` may enter TurnRun through `x38`;
  - if `mv.co.run.x0 > 0`, Run IASA returns;
  - otherwise RunBrake may start when `abs(lstick.x) < x58`.
- `ftCo_Run_Phys` applies run accel, `x5C` taper, stores
  `mv.co.run.x4 = target_vel * x440`, and uses run friction `x60`.
- `ftCo_Run_Coll` uses `ft_800844EC`.

Rust:
- Run lives in `sim.rs:180-204`.
- `is_same_direction_run` uses extracted `x58`.
- `is_opposite_run_turn` uses extracted `x38`.
- `run_no_interrupt_frames` models `x430`.
- `apply_run_velocity` implements dash/run accel, target, and `x5C` taper.
- Graph marks Run partial.

Differences:
- `run_no_interrupt_frames` decrements inside the Run state branch. In source,
  `mv.co.run.x0` is decremented in `Run_Anim` before `Run_IASA`. If the counter
  reaches zero on a frame, source can evaluate TurnRun/RunBrake on that same
  frame; Rust waits until the next tick.
- Rust does not carry `mv.co.run.x4` for animation-rate behavior.
- Rust has no source animation-rate feedback into visible Run/RunDirect.
- Run collision uses simplified floor support.

Impact:
- High around turnaround follow-through timing and the first actionable frame
  after `TurnRun -> Run`.

### RunBrake

Source:
- `ftCo_RunBrake_CheckInput` enters RunBrake when `abs(lstick.x) < x58`.
- `ftCo_RunBrake_Enter` clears `cmd_vars[0]` and `cmd_vars[1]`, stores
  `max_run_brake_frames`.
- `ftCo_RunBrake_Anim` can pause/resume animation around PlCo `x42C` when
  `cmd_vars[1]` is set, decrements brake frames, and falls back through
  `ft_8008A2BC` when animation/frames are done.
- `ftCo_RunBrake_IASA` allows jump, then `cmd_vars[0] && fn_800C9CEC`, then
  `ftCo_800D5FB0`.
- `fn_800C9CEC` enters TurnRun through the `x38` threshold and starts TurnRun at
  the current animation frame.
- `ftCo_RunBrake_Phys` uses run friction `x60`.
- `ftCo_RunBrake_Coll` uses `ft_80084280`.

Rust:
- RunBrake lives in `sim.rs:230-248`.
- Rust covers jump, crouch, run friction, zero-velocity exit, and extracted
  Falcon max brake frames.
- Graph marks RunBrake and `Run -> RunBrake` partial.

Differences:
- Rust does not model `cmd_vars[0]`, so RunBrake cannot source-transition into
  TurnRun through `fn_800C9CEC`.
- Rust does not model `cmd_vars[1]` or PlCo `x42C` animation pause/resume.
- Rust exits to Wait when velocity reaches zero or max brake frames expire.
  With held stick, the next Wait tick can become Walk. That is a plausible
  route for the reported "run turnaround just becomes walk" symptom when the
  source would still allow a RunBrake-to-TurnRun branch.

Impact:
- High. This is the clearest missing source transition adjacent to the user's
  run-turnaround complaint.

### TurnRun

Source:
- `fn_800C9D40` enters TurnRun from Run through `x38`.
- `ftCo_TurnRun_Enter` clears `cmd_vars[1]`, stores old-facing
  `mv.co.turnrun.accel_mul`, changes state with `Ft_MF_SkipAnimVel`, and clears
  `x14`.
- `ftCo_TurnRun_Anim`:
  - if `cmd_vars[1]` is set, first pauses anim rate and sets `x14`;
  - later resumes animation and flips facing when
    `mv.co.walk.middle_anim_frame * gr_vel <= 0.01`;
  - on animation completion, calls `fn_800CA644` to enter Run if same-facing
    stick reaches `x58`; otherwise falls back through `ft_8008A2BC`.
- `ftCo_TurnRun_IASA` only allows jump.
- `ftCo_TurnRun_Phys` accepts opposite acceleration while
  `accel_mul * accel < 0`, otherwise applies run friction.
- `ftCo_TurnRun_Coll` handles floor loss and edge behavior.

Rust:
- TurnRun lives in `sim.rs:250-271`.
- Entry stores `turn_run_accel_mul`, applies source-shaped TurnRun physics on
  the entry tick, and preserves old facing until velocity crosses zero.
- `advance_turn_run_facing` flips facing when velocity crosses zero.
- Rust enters Run immediately after `turn_has_turned`, same-direction velocity,
  and same-facing `x58` run input.
- Graph marks TurnRun and `TurnRun -> Run` partial.

Differences:
- Rust does not model `cmd_vars[1]`, `mv.co.turnrun.x14`, animation-rate pause,
  or animation-completion-gated `fn_800CA644`.
- Rust flips facing from velocity crossing in the physics/result path; source
  flips facing in the animation callback when the command-var gate says it can.
- Rust can enter Run before the source animation completion point, or miss the
  source's fallback timing.
- Rust's `TurnRun -> Wait` fallback is only velocity-zero plus neutral stick;
  source fallback is animation-completion plus failed `fn_800CA644`.

Impact:
- High. This is the other major gap for run turnaround feel, dash-dance miss
  recovery, and "run back and forth with turnarounds between run states."

### RunDirect

Source:
- `ftCo_RunDirect_Anim` and Phys/Coll delegate to Run.
- `ftCo_RunDirect_IASA` includes the same action branches, then
  `fp->mv.ca.specials.grav <= 0.0F && fn_800CA698`, then `ft_8008A244`.
- Local decomp search found the state table entry and callbacks, but no normal
  gameplay entry path into RunDirect.

Rust:
- RunDirect is a distinct `MotionState` for diagnostics, checksums, visuals,
  ECB lookup, and injected/replay state identity.
- Rust models same-facing handoff to Run and fallback to Wait for injected
  RunDirect.
- Graph marks RunDirect partial.

Differences:
- No source-backed normal entry path has been found yet.
- `mv.ca.specials.grav` is not represented as a real RunDirect gate outside the
  diagnostic state identity.

Impact:
- Low for current playtest unless replays inject RunDirect or a later source
  audit finds its real entry path.

## Priority Gap Queue

| Priority | Gap | Source anchor | Rust anchor | Why it matters | Next evidence/test |
| --- | --- | --- | --- | --- | --- |
| P0 | RunBrake cannot branch into TurnRun through `cmd_vars[0] && fn_800C9CEC` | `ftCo_RunBrake.c:85-92`, `ftCo_TurnRun.c:21-31` | `sim.rs:230-248` | Likely route for held reverse input degrading into Wait/Walk after a missed run-turn timing | Add a source-shaped test where Run enters RunBrake, holds opposite past x38 during the command-var window, and must reach TurnRun/Run rather than Wait/Walk |
| P0 | TurnRun exit is velocity-crossing based instead of animation-command/completion based | `ftCo_TurnRun.c:60-78`, `ftCo_Run.c:40-50` | `sim.rs:250-271`, `sim.rs:1266` | Core run-turnaround follow-through can happen too early, too late, or via wrong fallback | Add tests around TurnRun frames: no Run before source completion, Run on completion with x58, Wait fallback on completion without x58 |
| P0 | Run no-interrupt counter is decremented in the state branch, not source Anim before IASA | `ftCo_Run.c:81-104`, `ftCo_Run.c:106-132` | `sim.rs:180-204` | Can shift the first actionable TurnRun/RunBrake frame after `x430` by one tick | Add x430 boundary test with counter == 1 and reverse input; source should evaluate after Anim decrement |
| P1 | Dash exit uses fixed Rust frame logic instead of `cmd_vars[0]` and Anim completion | `ftCo_Dash.c:76-135` | `sim.rs:136-178`, `sim.rs:1983` | One-frame differences can affect dash dance, moonwalk, and Dash-to-Run feel | Add frame trace comparing Dash command-var run gate against current `profile.dash_frames` exit |
| P1 | Walk animation bucket remap and `mv.co.walk.x0` are not modeled | `ftwalkcommon.c:98-166` | `sim.rs:1038`, `sim.rs:1900` | Visual walk/run feel and bucket switches can look wrong despite correct displacement | Add walk bucket transition tests that verify animation frame/rate once animation vars exist |
| P1 | Turn temporary-facing IASA and button latch are approximated | `ftCo_Turn.c:99-150` | `sim.rs:273-320` | Dashback and turnaround attack/special edge cases can differ | Add narrow tests for pre-turn and post-turn action priority with latched A/B |
| P1 | Runtime trace lacks hidden locomotion vars | `readout.rs:224-480` | `state.rs:539-552` | Hard to diagnose user playtests that say "it walked out" when state labels/velocity are not enough | Add trace fields for `ground_velocity_x`, accels, dash delta, run no-interrupt, turn flags, and turn-run accel mul |
| P2 | Collision helpers are simplified for ledges/edges | `ft_081B.c:1069-1142` | `sim.rs:692-699` | Not urgent on center Battlefield, important near ledges | Later edge/ledge movement parity pass |
| P2 | Wait/Walk item, metal, scale, and special-case helpers are outside current Rust slice | `ftCo_Wait.c`, `ftCo_Walk.c` | `sim.rs:780`, `sim.rs:1900` | Not current Falcon no-item sandbox, but needed for completeness | Track separately from movement feel work |

## Current User-Facing Expectation

Based on this audit, the current engine should not be expected to have full
human-noticeable grounded movement parity yet.

What should be reasonably testable:

- Dash starts, Dash-to-Run, basic Run, Run-to-TurnRun through x38, and a core
  full-stick `Run -> TurnRun -> Run` chain have test coverage.
- Moonwalk-like dash payload behavior has tests, but the graph still notes the
  human feel is under review.
- Value and ECB data for the current Falcon movement sandbox are in good shape.

What should not be expected yet:

- Frame-perfect RunBrake-to-TurnRun behavior after a missed timing.
- Source-accurate TurnRun animation pause, facing flip, and Run exit timing.
- Source-accurate animation-rate feel for Walk/Run/RunDirect.
- Full grounded edge/ledge collision behavior.

## Recommended Next Slice

Do not tune speeds or add gameplay states.

The next implementation slice should be:

1. Add source-shaped motion vars needed for RunBrake and TurnRun:
   `cmd_vars[0]`, `cmd_vars[1]`, RunBrake `x0`, RunBrake frame timer,
   TurnRun `x14`, and a source-equivalent animation-completion gate.
2. Add failing tests for:
   - RunBrake command-var-gated `fn_800C9CEC`.
   - TurnRun completion-gated `fn_800CA644`.
   - `x430` counter decrement before Run IASA.
3. Keep the Rust core deterministic and vanilla.
4. Update state graphs, value refs, parity report notes, and runtime trace fields
   when behavior changes.

For live playtest diagnosis, use:

```powershell
.\execs\Run SDL3 Runtime Vanilla No UCF.cmd
```

or run the runtime with `--input-trace`. The trace currently records WUP raw,
native, UCF, mapped input, before/after motion state, facing, velocity, and core
input facts. It does not yet expose all hidden locomotion vars listed in the P1
tooling gap.
