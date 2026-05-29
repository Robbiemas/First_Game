# Melee Data Provenance Audit

Date: 2026-05-28

This project should copy Melee-shaped logic and consume Melee-extracted data.
It should not tune gameplay values by feel when a decomp or DAT-backed value is
available.

## Local Reference State

Available under `D:\Mole Game\.research`:

- `doldecomp-melee`: local Melee decomp source tree
- `dolphin`: Dolphin source reference
- `slippi`: Slippi/Dolphin fork reference

The local `doldecomp-melee` checkout currently provides source code, struct
layouts, field offsets, and DAT filename references such as `PlCo.dat` and
`PlCa.dat`. The local research folders do not vendor extracted DAT files, but a
local Melee 1.02 disc image was used to extract `PlCo.dat` and `PlCa.dat` into
the ignored `resources/melee/raw` folder. Reviewable JSON snapshots generated
from those local bytes are checked in under `resources/melee/extracted`.

The repo now has a temporary resource bootstrap at `resources/melee`: raw
user-provided DAT files go in `resources/melee/raw` and are ignored by git, while
`tools/extract_melee_resources.py` writes small reviewable JSON snapshots into
`resources/melee/extracted`. The extractor follows the HSD DAT root-node layout:
`PlCo.dat` resolves `ftLoadCommonData -> CommonAttributes`, and `PlCa.dat`
resolves `ftDataCaptain.x0 -> ftCo_DatAttrs`.

## Current Decomp-Shaped Logic

- Ground jump takeoff follows `ftCo_Jump.c`: full hop and short hop select
  profile-owned vertical takeoff forces instead of solving velocity from height.
- Air jump entry follows `ftCo_JumpAerial.c`: vertical force is profile-owned and
  represents `jump_v_initial_velocity * air_jump_v_multiplier`.
- Normal fall follows `ftCommon_Fall`: subtract gravity and clamp to terminal
  fall speed.
- Fast fall follows `ftCommon_FallFast`: set vertical velocity directly to
  `-fast_fall_velocity`.
- Current/previous input snapshots, tap timers, and checksums are rollback-owned
  inside Rust core state.
- `World` now carries `MeleeCommonData`, mixes it into rollback checksums, and
  uses that world-owned data for input tap thresholds, input-fact thresholds,
  fast-fall gates, aerial-jump forward/back selection, crouch entry/release
  gates, shield platform-pass gates, pass/drop-through initial velocity, dash
  action windows, run thresholds, and the EscapeAir/FallSpecial common-data
  slice. The EscapeAir/FallSpecial subset now consumes extracted `PlCo.dat`
  bootstrap values rather than guessed Mole constants.
- `MeleeCommonData::from_plco_bytes` now extracts the run/run-brake stick
  threshold `x58_someLStickXThreshold`; the simulator's run and TurnRun routing
  reads the centralized value through `MeleeCommonData` instead of a local
  hardcoded constant.
- `MeleeCommonData::from_plco_bytes` now extracts common-data `x6C`, the
  high-speed ground-friction multiplier used by `ft_80084F3C` when ground
  velocity exceeds `walk_max_vel`; wavedash landing slide friction now reads
  this value from world-owned common data.
- `FighterProfile::from_ftco_dat_attrs_bytes` can now read one-to-one
  `ftCo_DatAttrs` fields from a big-endian character attribute byte slice:
  `walk_accel`, `walk_max_vel`, `gr_friction`, `dash_initial_velocity`,
  `dash_run_acceleration_a`, `dash_run_acceleration_b`,
  `dash_run_terminal_velocity`, `max_run_brake_frames`,
  `ground_max_horizontal_velocity`, `jump_startup_time`,
  `jump_h_initial_velocity`, `jump_v_initial_velocity`,
  `ground_to_air_jump_momentum_multiplier`, `jump_h_max_velocity`,
  `hop_v_initial_velocity`, `air_jump_v_multiplier`,
  `air_jump_h_multiplier`, `max_jumps`, `grav`, `terminal_vel`,
  `air_drift_stick_mul`, `aerial_drift_base`, `air_drift_max`,
  `aerial_friction`, `fast_fall_velocity`, and
  `air_max_horizontal_velocity`,
  `frames_to_change_direction_on_standing_turn`, and
  `normal_landing_lag`.
- Normal air drift follows `ftcommon.c`'s source-shaped `air_drift_stick_mul +
  aerial_drift_base` acceleration toward `air_drift_max`, with
  `aerial_friction` used when the target is zero or would be overshot.
- Dash/run acceleration follows the `getAccelAndTarget` helper:
  main-stick X scales `dash_run_acceleration_a`, same-side input adds
  `dash_run_acceleration_b`, and the target velocity scales
  `dash_run_terminal_velocity`.
- Basic standing-turn direction-change timing is profile-owned through
  `ftCo_DatAttrs.frames_to_change_direction_on_standing_turn`.
- Standing-turn total duration is now profile-owned through
  `FighterProfile::standing_turn_total_frames`; the fallback Falcon-like value
  remains an animation-data placeholder until exact action data is extracted.
- Attack1 and AttackDash total durations/IASA, plus `GuardOn`, `GuardOff`,
  `EscapeN`, `EscapeF`, `EscapeB`, `Squat`, and `SquatRv` total durations, now
  read through `FighterActionFrames`, which is carried by `FighterProfile` and
  mixed into the rollback checksum. The Falcon-like values are still fallback
  frame-data values until extracted action data is available.
- Run-brake max duration now comes from extracted Captain Falcon
  `ftCo_DatAttrs.max_run_brake_frames` in both the default Falcon-like profile
  and the generated DAT-backed profile path.
- Ordinary `Landing` duration is profile-owned through
  `ftCo_DatAttrs.normal_landing_lag` instead of a simulation constant.
- Landing state selection now uses a Rust-owned stage-contact helper over
  `StageProfile` surfaces instead of a simulator-local hardcoded floor snap.
  This keeps air-dodge landing movement deterministic and prepares the collision
  path for platform/drop-through parity.
- Grounded movement now rechecks `StageProfile` floor support after horizontal
  translation and enters ordinary `Fall` when the ECB bottom leaves the floor
  span, instead of carrying grounded state beyond the edge.
- Profile-owned stick-scaled movement now treats native main-stick `127` as
  Melee's `1.0` stick magnitude for jump horizontal velocity, air drift, and
  dash/run acceleration targets. This keeps extracted `ftCo_DatAttrs` values in
  Melee units instead of percent-style `100` scaling.
- Extracted `escapeair_decay` now stays as a milli fixed-point multiplier in
  `MeleeCommonData`, so air-dodge self-velocity decay can preserve source float
  precision instead of rounding the common-data field to whole percent steps.
- The default Falcon-like movement profile now uses the generated Captain
  Falcon bootstrap values for walk acceleration, dash/run acceleration, run
  brake frames, ground/air horizontal velocity caps, jump horizontal velocity,
  air-jump horizontal velocity, standing-turn direction-change timing, and
  landing lag. The DAT `max_jumps` value is treated as Melee's total jump count;
  Rust stores the remaining aerial jumps after a grounded jump.

## Active Data Gaps

These should not be hand-tuned:

- Full Melee collision parity still needs ledges, walls, ceilings, cliff catch,
  pass-through platform timing, and source-accurate collision callbacks.
- `MeleeCommonData::PROVISIONAL` still has broad input thresholds that should be
  replaced or verified field-by-field against `PlCo.dat`; the air-dodge subset
  is now extracted.
- `FighterProfile::FALCON_LIKE` still needs extracted action/submotion data for
  animation-specific durations and IASA. Core movement attributes now use the
  generated Captain Falcon bootstrap values.
- Animation durations and IASA frames currently stored as `FALCON_*` constants:
  replace with extracted action/animation data. Standing turn, Attack1,
  AttackDash, GuardOn, GuardOff, spotdodge, rolls, crouch startup, and crouch
  release now have profile-owned fields, but their fallback values are still not
  extracted from animation/action data.
- Exact air-dodge animation length still needs submotion/animation extraction;
  `x334` is the source action timer and must not be used as the total animation
  duration.

## Working Rule

When a value is missing, keep it labeled as provisional or as a data gap. Do not
silently replace it with a guessed number. When a value is available from local
decomp code or extracted DAT bytes, add a test that names the source field or
motion-state path and locks the Rust behavior to that source.
