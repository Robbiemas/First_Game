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

- Value parity: 103/103 matching rows, 64 global and 39 Falcon/test-character.
- Falcon ECB coverage: 70 mapped motion states, 0 missing sampled mappings,
  0 unmapped derived states.
- State graph: 52 nodes, 73 edges, 24 aligned, 3 intentional, 98 partial.
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
| Input thresholds | PlCo fields such as `x24`, `x34`, `x38`, `x3C`, `x40`, `x44`, `x48`, `x4C`, `x54`, `x58`, `x5C`, `x60`, `x42C`, `x430` | Extracted through `MeleeCommonData`; value ledger is 103/103 | Mostly aligned |
| Dash/run accel and target | `getAccelAndTarget` in `inlines.h:130-137`: HSD-clamped fighter stick `f32` times source `f32` acceleration/terminal velocity | `fighter_stick_axis_to_f32` plus `dash_run_accel_and_target` in `sim.rs` | Aligned for the current integer-position bridge; grounded source floats are stored and hashed by raw bits |
| Ground accel toward target | `ftCommon_8007C98C` in `ftcommon.c:71-105` | `apply_ground_accel_toward_target` in `sim.rs` | Aligned for flat-ground source-float acceleration and zero-crossing friction behavior in the current grounded slice |
| General ground friction | `ft_80084F3C` in `ft_084E.c:41-52` with high-speed multiplier over walk max | `apply_ground_traction` in `sim.rs:2009` | Mostly aligned for flat Battlefield |
| Run friction | run/turn/brake use `gr_friction * x60` | `run_ground_friction` in `sim.rs:2027` | Aligned in value, simplified surface multiplier |
| Ground movement projection | `ftCommon_ApplyGroundMovement` in `ftcommon.c:143-160` projects through floor normal into anim/self velocity | Rust now keeps flat-stage source-float `ground_velocity_x`, `ground_accel_x`, and `ground_accel_x2`, then converts to milli only at the position/render bridge | Partial: Battlefield main floor ok, slopes/normal projection and source-float position are not complete |
| Ground collision | `ft_80084280` and `ft_800844EC` in `ft_081B.c:1069-1142` handle ledges, nudges, fall, edge behavior | `has_floor_support` plus fall transition after position commit | Partial, acceptable on center Battlefield floor only |
| Motion command vars | source `cmd_vars[0]`, `cmd_vars[1]` gate Dash, RunBrake, TurnRun behavior | Dash, RunBrake, and TurnRun command vars are deterministic Rust fields driven by extracted Falcon action-script events | Mostly aligned for current grounded slice |
| Animation-frame gates | source `cur_anim_frame`, `ftAnim_IsFramesRemaining`, anim rate, and motion vars gate exits | Walk, Run, Dash, RunBrake, and TurnRun carry source-shaped motion vars/rate gates for the current grounded slice | Mostly aligned for flat-ground Falcon locomotion |

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
- `walk_anim_tick` carries the source-shaped animation-rate update and
  remap timing.
- `apply_walk_velocity` in `sim.rs:1900` implements source-shaped initial
  velocity, accel, target, taper, and `mv.co.walk.x0`-style animation
  velocity storage.
- Graph marks WalkSlow/Middle/Fast aligned.

Differences:
- Exact source animation-frame seed values passed through `ftCo_Walk_Enter`
  still use the current Rust frame counter rather than extracted per-bucket
  animation-frame constants.
- Metal/scale/item held movement multipliers in `ftCo_Walk_Enter` are absent.
- Input debug buckets still have provisional `walk_slow_x`, `walk_middle_x`,
  and `walk_fast_x` values. State selection itself is velocity-based.

Impact:
- Low for raw flat-ground displacement.
- Lower than before this slice for visible animation feel and walk bucket
  transitions; remaining gaps are outside the no-item flat-ground Falcon
  sandbox or need exact per-bucket animation-frame constants.

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
- `turn_effective_input_facts` now applies the source temporary-facing action
  pass before grounded attack resolution, so pre-flip old-forward soft A falls
  through to jab instead of becoming a side tilt.
- The runtime input trace exposes Turn's hidden facing/latch vars alongside
  the controller samples.
- UCF dashback amendment remains adapter-owned and is consumed at the source
  Turn hook point as a flag.
- Graph marks Turn partial.

Differences:
- Rust has the source-shaped temporary-facing action pass and A/B latch fields,
  but not every source helper called from `ftCo_Turn_IASA` has a one-to-one
  Rust identity yet.
- Turn collision remains simplified relative to `ft_80083F88`.

Impact:
- Lower than before this slice for turnaround attack direction edge cases.
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
- `dash_anim_tick` applies extracted Falcon Dash action-script `cmd_vars[0]`
  events: clear on frame 0, set on frame 16, and animation-completion fallback
  on frame 29.
- `apply_dash_physics` consumes `mv.co.dash.x0` on the first Dash physics tick,
  then later frames use live-stick dash/run accel and target.
- Dash-to-Run applies the Run physics helper on the same handoff frame, matching
  source callback order where `Fighter_procUpdate` follows `Dash_IASA` with the
  updated Run `phys_cb`.
- `apply_dash_velocity` and `dash_iasa_decayed_ground_velocity` cover live dash
  accel and x54 decay.
- Graph marks Dash and `Dash -> Run` aligned.

Differences:
- Dash collision/edge handling is simplified.

Impact:
- Lower than before this slice for Dash-to-Run and non-run Dash fallback timing.
  Remaining Dash risk is mostly collision/edge handling and still-uncovered
  late-window action branches.

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
- `run_anim_tick` decrements `run_no_interrupt_frames` before IASA, matching
  `ftCo_Run_Anim -> ftCo_Run_IASA`.
- `apply_run_velocity` implements dash/run accel, target, and `x5C` taper.
- Graph marks Run partial.

Differences:
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
- RunBrake lives in the `MotionState::RunBrake` branch.
- `run_brake_anim_tick` models the Falcon action-script `cmd_vars[0]` window,
  the `cmd_vars[1]`/PlCo `x42C` animation pause slot, and extracted
  `max_run_brake_frames`.
- IASA now checks jump, then `cmd_vars[0] && fn_800C9CEC`-shaped opposite
  TurnRun, then squat/braking fallback.
- Graph marks `RunBrake -> TurnRun` aligned for this locomotion path while the
  broader RunBrake state remains partial.

Differences:
- RunBrake collision remains simplified relative to `ft_80084280`.
- Non-locomotion interrupts outside jump/squat/TurnRun still need the broader
  source-callback audit.

Impact:
- Lower than before this slice for run-turnaround miss recovery; still medium
  until the wider callback/collision pass is complete.

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
- TurnRun lives in the `MotionState::TurnRun` branch.
- Entry stores `turn_run_accel_mul`, applies source-shaped TurnRun physics on
  the entry tick, and preserves old facing until velocity crosses zero.
- `turn_run_anim_tick` models the Falcon action-script `cmd_vars[1]` frame,
  animation pause slot `x14`, velocity-crossing resume/facing flip, and
  completion-gated `fn_800CA644`-style Run handoff.
- Graph marks `TurnRun -> Run` aligned for this grounded locomotion path while
  the broader TurnRun state remains partial.

Differences:
- The velocity-crossing resume uses the extracted source shape, but floor-normal
  projection remains simplified in Rust's flat-Battlefield physics.
- TurnRun collision remains simplified relative to the source collision
  callback.

Impact:
- Lower than before this slice for run-turnaround follow-through; remaining
  differences are now more likely in Dash command-var timing, Turn temporary
  facing/latches, Walk animation vars, or collision/projection.

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
| Done P0 | RunBrake command-var TurnRun branch | `ftCo_RunBrake.c:85-92`, `ftCo_TurnRun.c:21-31` | `MotionState::RunBrake`, `run_brake_anim_tick` | Prevents held reverse input from falling through to Wait/Walk when source still allows TurnRun | Covered by `run_brake_cmd_var0_window_can_branch_to_turnrun_before_wait_or_walk` |
| Done P0 | TurnRun completion-gated Run handoff | `ftCo_TurnRun.c:60-78`, `ftCo_Run.c:40-50` | `MotionState::TurnRun`, `turn_run_anim_tick` | Stops velocity crossing from directly entering Run before source animation completion | Covered by `turn_run_does_not_enter_run_before_source_animation_completion` and `turn_run_completion_enters_run_through_source_x58_gate` |
| Done P0 | Run no-interrupt counter decrement before IASA | `ftCo_Run.c:81-104`, `ftCo_Run.c:106-132` | `MotionState::Run`, `run_anim_tick` | Keeps the first actionable TurnRun/RunBrake frame after `x430` on the source frame | Covered by `run_x430_decrements_before_iasa_allows_turnrun_on_boundary_frame` |
| Done P1 | Dash command-var Run gate, same-frame Run physics, and animation-completion fallback | `ftCo_Dash.c:76-163`, `fighter.c:2120-2161` | `MotionState::Dash`, `dash_anim_tick`, `apply_dash_physics`, `apply_run_velocity` | Removes the fixed-frame shortcut from Dash-to-Run and keeps the handoff frame on the source callback order | Covered by `dash_holding_forward_waits_for_source_cmd_var0_before_run_gate`, `dash_neutral_falls_back_on_animation_completion_not_profile_dash_frames`, `dash_entry_x0_suppresses_first_normal_dash_physics_tick`, and `run_acceleration_uses_source_x5c_remaining_velocity_taper` |
| Done P1 | Walk animation bucket remap and `mv.co.walk.x0` source fields | `ftwalkcommon.c:98-166` | `walk_anim_tick`, `walk_motion_state`, `apply_walk_velocity` | Keeps walk bucket transitions/rate fields source-shaped instead of only moving the character correctly | Covered by `walk_records_source_x0_and_anim_rate_from_bucket_velocity`, `walk_bucket_remap_preserves_source_motion_frame_and_resets_change_state_rate`, and checksum/snapshot coverage |
| Done P1 | Turn temporary-facing action pass and latch trace fields | `ftCo_Turn.c:99-150` | `turn_effective_input_facts`, `apply_turn_temporary_facing_to_attack_facts`, runtime trace | Prevents old-facing soft A during pre-flip Turn from resolving as forward side tilt; exposes hidden Turn vars for controller-log diagnosis | Covered by `turn_pre_flip_old_forward_tilt_uses_temporary_source_facing_for_attack_checks`, existing Turn latch tests, and trace-field coverage |
| Done P1 | Runtime trace lacked hidden locomotion vars | `readout.rs`, `mole_runtime/src/lib.rs` | `PlayerRenderSnapshot`, `RenderFrame`, `core_player_to_json` | Gives controller logs enough state/velocity context for playtest diagnosis | Trace now includes ground velocity/accels, dash delta, run no-interrupt, cmd vars, RunBrake fields, TurnRun pause, and animation rate |
| Done P2 | Replay oracle report collapsed source and core frame numbers | Slippi negative-frame export, `slippi_diagnostic.rs` | `SlippiCoreMismatch::source_frame`, report deltas | Prevents match-start reports from pointing at a core-frame index when the actionable source replay frame differs | Covered by `slippi_match_start_report_distinguishes_core_frame_from_source_replay_frame`; current replay export has unsupported state count `0`, P1 source frames `-29..-24` matching GuardReflect-to-Pass, first remaining position drift at P2 source frame `2` during JumpAerialF, and first remaining state mismatch at P2 source frame `99` where Rust remains LandingFallSpecial while Slippi has Fall |
| Done P2 | Fighter stick normalization used legacy percent/asymmetric scaling for dash/run helper paths | `controller.c` `HSD_PadScale`, `fighter.c` input copy, `inlines.h::getAccelAndTarget` | `fighter_stick_axis_to_f32`, `dash_run_accel_and_target`, value sheets | Keeps full HSD-clamped left/right stick at `-1.0/+1.0` and compares Falcon dash/run attrs as source `f32` values instead of old milli aliases | Covered by HSD `/127.0` stick tests, dash target/accel tests, and Slippi P2 dash-to-kneebend trace matching frames -20..-13 exactly |
| Done P2 | Grounded PlCo scalar fields used `_milli` aliases for source floats | PlCo `x28`, `x2C`, `x30`, `x54`, `x5C`, `x60`, `x6C`, `x42C`, `x440` | `MeleeCommonData`, `extract_melee_resources.py`, value sheets | Preserves exact extracted `f32` values for grounded ratios, tapers, friction multipliers, Dash decay, RunBrake pause velocity, and animation velocity scale | Covered by `grounded_common_scalars_remain_source_f32_not_milli_aliases`, parity value-sheet tests, and checksum hashing via `mix_f32` |
| Done P2 | Remaining exposed PlCo and Falcon movement floats used milli/extracted aliases | PlCo `escapeair_force`, `escapeair_decay`, `x444`, `x448`, `x46C`, `x6C4`; Falcon `ftCo_DatAttrs` movement floats | `MeleeCommonData`, `FighterProfile`, extractor/value sheets | Keeps EscapeAir, fall animation blend/threshold, pass Y velocity, entry scale, and Falcon walk/jump/air profile values in source `f32` form instead of per-tick milli aliases | Covered by source-float profile/common extraction tests, value sheet/parity tests, and checksum bit coverage |
| Done P2 | Grounded locomotion collapsed source `f32` velocity/accel through per-frame milli rounding | `fighter.c`, `ftcommon.c`, `ftCo_Dash.c`, `ftCo_Run.c`, `ftCo_Walk.c` source `gr_vel`, `xE4_ground_accel_1`, `xE8_ground_accel_2` float paths | `PlayerState` grounded motion fields, `commit_ground_velocity`, runtime trace | Keeps sub-milli source acceleration alive across frames, hashes the exact float bits for rollback, and avoids tapering acceleration from a true dead stop | Covered by `dash_ground_velocity_accumulates_source_float_sub_milli_accel`, source-float walk/run/dash expected-value tests, and snapshot/checksum coverage |
| P2 | Collision helpers are simplified for ledges/edges | `ft_081B.c:1069-1142` | `sim.rs:692-699` | Not urgent on center Battlefield, important near ledges | Later edge/ledge movement parity pass |
| P2 | Wait/Walk item, metal, scale, and special-case helpers are outside current Rust slice | `ftCo_Wait.c`, `ftCo_Walk.c` | `sim.rs:780`, `sim.rs:1900` | Not current Falcon no-item sandbox, but needed for completeness | Track separately from movement feel work |

## Current User-Facing Expectation

Based on this audit, the current engine is closer but still should not be
treated as complete human-noticeable grounded movement parity.

What should be reasonably testable:

- Dash starts, Dash-to-Run, basic Run, Run-to-TurnRun through x38, RunBrake
  command-var TurnRun, source-shaped TurnRun completion, and the x430 boundary
  have test coverage.
- Moonwalk-like dash payload behavior has tests, but the graph still notes the
  human feel is under review.
- Value and ECB data for the current Falcon movement sandbox are in good shape.

What should not be expected yet:

- Remaining Dash late-window action branches and edge collision behavior.
- One-to-one helper identity for every Turn action branch and full Turn
  collision/edge behavior.
- Full source animation-rate feel for Run/RunDirect and exact Walk per-bucket
  seed frames.
- Full grounded edge/ledge collision behavior.

## Recommended Next Slice

Do not tune speeds or add gameplay states.

The next implementation slice should be:

1. Investigate the current replay oracle's earliest remaining drift, now around
   P1 `Pass`/platform drop horizontal velocity, before tuning grounded feel.
2. Continue the float-domain migration at the remaining public gameplay bridges
   such as position/velocity and Slippi/debug reporting, only where the decomp
   uses floats, while hashing exact float bits for deterministic replay.
3. Add failing tests around the chosen source callback behavior before changing
   implementation.
4. Keep the Rust core deterministic and vanilla; UCF stays in the input/replay
   adapter layer.
5. Update state graphs, value refs, parity report notes, and runtime trace
   fields when behavior changes.

For live playtest diagnosis, use:

```powershell
.\execs\Run SDL3 Runtime Vanilla No UCF.cmd
```

or run the runtime with `--input-trace`. The trace currently records WUP raw,
native, UCF, mapped input, before/after motion state, facing, velocity, hidden
ground locomotion vars, command vars, animation-rate fields, and core input
facts.
