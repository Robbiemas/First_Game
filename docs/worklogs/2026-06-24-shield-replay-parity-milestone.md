# Shield And Replay Parity Milestone - 2026-06-24

## Status

This historical checkpoint is superseded by the scoped
[Captain Falcon/Battlefield full-replay parity milestone](../release_notes/2026-07-15-captain-falcon-battlefield-full-replay-parity.md).
Its notes remain useful provenance, but it is no longer the current replay
status.

- Shield collision/render export is gated by the decomp-backed `x221B_b0`
  equivalent, not by stale shield object position.
- Raptor Boost detect, DownBound airborne transition, and Fall entry now keep
  `self_vel`, `gr_vel`, and damage knockback separated instead of collapsing
  them into one convenience velocity.
- Replay scan over the currently exported Slippi input range reports
  `engine_root_scenario_count = 0` through 3200 frames.

## Kept As Active Work

The dirty tracked files in `crates/`, `resources/melee/`, `tools/`, `tests/`,
`docs/`, and `execs/` are current milestone work or generated parity artifacts.
They should not be deleted as cleanup without a source-backed replacement.

Large expanded/sample artifacts remain temporarily because tests and parity
proofs still reference them while the compact runtime data path is being
completed:

- `resources/melee/extracted/captain_falcon_action_ecb_samples.json`
- `resources/melee/extracted/captain_falcon_action_animation_table.json`
- `crates/mole_core/src/generated/falcon_ecb.rs`

These are still classified as debug/legacy-expanded artifacts, not final
runtime architecture.

## Cleaned

Removed ignored/generated local output only:

- Python bytecode/cache folders: `__pycache__`, `.pytest_cache`
- Old local controller/netplay logs under `logs/`
- Old replay trace/explain/frame-log files under `debug/slippi/`
- Duplicate debug scratchpad `debug/parity_scratchpad.md`
- Older full replay input/report copies superseded by the current exported
  `debug/slippi/Game_20260530T214929.inputs.json`

Approximate removed size: 147 MiB.

## Guardrail

Do not use cleanup as a way to hide parity gaps. A file can be removed only
when it is generated/transient, ignored output, or has a verified source-backed
replacement in the current Rust/decomp path.
