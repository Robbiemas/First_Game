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
- `FighterProfile::from_ftco_dat_attrs_bytes` can now read one-to-one
  `ftCo_DatAttrs` fields from a big-endian character attribute byte slice:
  `walk_accel`, `walk_max_vel`, `gr_friction`, `dash_initial_velocity`,
  `dash_run_acceleration_a`, `dash_run_acceleration_b`,
  `dash_run_terminal_velocity`, `ground_max_horizontal_velocity`,
  `jump_startup_time`,
  `jump_h_initial_velocity`, `jump_v_initial_velocity`,
  `ground_to_air_jump_momentum_multiplier`, `jump_h_max_velocity`,
  `hop_v_initial_velocity`, `air_jump_v_multiplier`,
  `air_jump_h_multiplier`, `max_jumps`, `grav`, `terminal_vel`,
  `air_drift_stick_mul`, `aerial_drift_base`, `air_drift_max`,
  `aerial_friction`, `fast_fall_velocity`, and
  `air_max_horizontal_velocity`, and `normal_landing_lag`.
- Normal air drift follows `ftcommon.c`'s source-shaped `air_drift_stick_mul +
  aerial_drift_base` acceleration toward `air_drift_max`, with
  `aerial_friction` used when the target is zero or would be overshot.
- Dash/run acceleration follows the `getAccelAndTarget` helper:
  main-stick X scales `dash_run_acceleration_a`, same-side input adds
  `dash_run_acceleration_b`, and the target velocity scales
  `dash_run_terminal_velocity`.
- Ordinary `Landing` duration is profile-owned through
  `ftCo_DatAttrs.normal_landing_lag` instead of a simulation constant.

## Active Data Gaps

These should not be hand-tuned:

- `MeleeCommonData::PROVISIONAL` values: replace with `PlCo.dat` extraction via
  `MeleeCommonData::from_plco_bytes`.
- `FighterProfile::FALCON_LIKE` character attributes: replace by feeding
  extracted Captain Falcon `ftCo_DatAttrs` bytes into
  `FighterProfile::from_ftco_dat_attrs_bytes` once `PlCa.dat` data is available.
- Animation durations and IASA frames currently stored as `FALCON_*` constants:
  replace with extracted action/animation data.
- Escape-air force, decay, deadzones, landing lag, and related common-data
  values: replace through `PlCo.dat` extraction.
- The default public `FighterProfile::FALCON_LIKE` values are still a fallback
  until extracted Captain Falcon bytes are available; the profile shape now has
  extraction slots for horizontal jump, air jump, max jumps, and air drift
  attributes, plus ordinary landing lag, instead of leaving those values as
  mechanics constants.

## Working Rule

When a value is missing, keep it labeled as provisional or as a data gap. Do not
silently replace it with a guessed number. When a value is available from local
decomp code or extracted DAT bytes, add a test that names the source field or
motion-state path and locks the Rust behavior to that source.
