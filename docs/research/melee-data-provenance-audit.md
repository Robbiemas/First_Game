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
  `dash_run_terminal_velocity`, `jump_startup_time`,
  `jump_v_initial_velocity`, `hop_v_initial_velocity`,
  `air_jump_v_multiplier`, `grav`, `terminal_vel`, and
  `fast_fall_velocity`.

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
- Air drift, jump horizontal velocity, ground-to-air momentum, and jump horizontal
  clamp values: route through extracted fighter attributes rather than local
  constants.

## Working Rule

When a value is missing, keep it labeled as provisional or as a data gap. Do not
silently replace it with a guessed number. When a value is available from local
decomp code or extracted DAT bytes, add a test that names the source field or
motion-state path and locks the Rust behavior to that source.
