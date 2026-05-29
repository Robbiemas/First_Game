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
`PlCa.dat`. A workspace search did not find extracted `PlCo.dat`, `PlCa.dat`, or
`ftDataCaptain` data files in the local research folders. That means source logic
can be copied now, while exact table values must either come from an extractor
fed by real DAT bytes or remain explicitly marked as data gaps.

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
  slice. This turns the existing `PlCo.dat` extractor seam into data the
  deterministic simulation can actually consume once clean bytes are available.
- `MeleeCommonData::from_plco_bytes` now extracts the run/run-brake stick
  threshold `x58_someLStickXThreshold`; the simulator's run and TurnRun routing
  reads the provisional value through `MeleeCommonData` instead of a local
  hardcoded constant.
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
- Run-brake max duration can now be profile-owned through
  `ftCo_DatAttrs.max_run_brake_frames`; the fallback profile keeps this unset
  until real Captain Falcon attribute bytes are available.
- Ordinary `Landing` duration is profile-owned through
  `ftCo_DatAttrs.normal_landing_lag` instead of a simulation constant.
- Landing state selection now uses a Rust-owned stage-contact helper over
  `StageProfile` surfaces instead of a simulator-local hardcoded floor snap.
  This keeps air-dodge landing movement deterministic and prepares the collision
  path for platform/drop-through parity.

## Active Data Gaps

These should not be hand-tuned:

- Full Melee collision parity still needs ledges, walls, ceilings, cliff catch,
  pass-through platform timing, and source-accurate collision callbacks.
- `MeleeCommonData::PROVISIONAL` values: replace with `PlCo.dat` extraction via
  `MeleeCommonData::from_plco_bytes`.
- `FighterProfile::FALCON_LIKE` character attributes: replace by feeding
  extracted Captain Falcon `ftCo_DatAttrs` bytes into
  `FighterProfile::from_ftco_dat_attrs_bytes` once `PlCa.dat` data is available.
- Animation durations and IASA frames currently stored as `FALCON_*` constants:
  replace with extracted action/animation data. Standing turn, Attack1,
  AttackDash, GuardOn, GuardOff, spotdodge, rolls, crouch startup, and crouch
  release now have profile-owned fields, but their fallback values are still not
  extracted from animation/action data.
- Escape-air force, decay, deadzones, landing lag, and related common-data
  values: replace through `PlCo.dat` extraction.
- The default public `FighterProfile::FALCON_LIKE` values are still a fallback
  until extracted Captain Falcon bytes are available; the profile shape now has
  extraction slots for horizontal jump, air jump, max jumps, air drift
  attributes, optional run-brake max frames, standing-turn direction-change
  timing, standing-turn total frames, Attack1/AttackDash action frames,
  defensive/crouch action-frame durations, and ordinary landing lag, instead of
  leaving those values as mechanics constants.

## Working Rule

When a value is missing, keep it labeled as provisional or as a data gap. Do not
silently replace it with a guessed number. When a value is available from local
decomp code or extracted DAT bytes, add a test that names the source field or
motion-state path and locks the Rust behavior to that source.
