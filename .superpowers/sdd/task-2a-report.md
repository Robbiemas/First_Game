# Task 2A Report: Mandatory Slippi Runtime Fixture

## Status

DONE_WITH_CONCERNS

Feature implementation commit: `2fecfce33dfbaa846098333e5450984ae36efee3` (`Make Slippi runtime fixture mandatory`)

## TDD Evidence

1. RED: added `slippi_match_start_fixture_is_tracked_and_complete` with an assertion that the tracked gzip fixture exists.
   - Command: `cargo test -p mole_runtime --test runtime_contract slippi_match_start_fixture_is_tracked_and_complete -- --exact`
   - Result: failed as expected because `crates/mole_runtime/tests/fixtures/Game_20260530T214929.inputs.json.gz` did not exist.
   - Counts: 0 passed, 1 failed, 0 ignored, 306 filtered out.

2. GREEN: added the tracked fixture, a `flate2` dev-dependency, the required gzip loader, explicit fixture expectations, and the parse/frame-count assertion.
   - The fixture was generated twice from `debug/slippi/Game_20260530T214929.inputs.json` with .NET `GZipStream`; both outputs had SHA-256 `D2A11D95E08E843F2EAD436561DE86259F13F11A5C0FD8272594D85390357F60`.
   - Source size: 18,550,825 bytes. Tracked gzip size: 500,274 bytes.
   - Focused command: `cargo test -p mole_runtime --test runtime_contract slippi_match_start_fixture_is_tracked_and_complete -- --exact`
   - Result: passed.
   - Counts: 1 passed, 0 failed, 0 ignored, 306 filtered out.

## Implementation

- `read_slippi_match_start_fixture_export` now returns `Result<String, String>` and reads only `tests/fixtures/Game_20260530T214929.inputs.json.gz` through `flate2::read::GzDecoder`.
- The fixture contract decompresses and parses the export, then asserts its stable 5,313 source-frame identity.
- Replaced all 95 `let Some(export) = ... else { return; };` fixture-absence success paths with explicit `expect` calls.
- No production gameplay files were edited.

## Files Changed

- `Cargo.lock`
- `crates/mole_runtime/Cargo.toml`
- `crates/mole_runtime/tests/runtime_contract.rs`
- `crates/mole_runtime/tests/fixtures/Game_20260530T214929.inputs.json.gz`

## Verification

- `cargo test -p mole_runtime --test runtime_contract`
  - Result: 276 passed, 31 failed, 0 ignored, 0 measured; finished in 476.84s.
- `cargo fmt --all -- --check`: passed.
- `git diff --check`: passed.
- The new focused fixture contract passed and was not in the full-suite failure list.

## Concerns

The full target has 31 existing behavioral/runtime expectation failures. They were not changed, relaxed, or hidden by this task:

- `attack100_loop_hit_pills_render_from_decoded_subroutine_script`
- `frame_debug_log_reports_canonical_source_hit_confirms`
- `looping_locomotion_source_capsules_wrap_baked_pose_frames`
- `render_scene_draws_translucent_bubble_shield_for_guard_states`
- `render_source_selection_reports_escape_air_action_44_for_authoritative_capsules`
- `render_source_selection_reports_landing_and_landing_air_aliases`
- `render_source_selection_reports_source_only_jab_grab_throw_capture_actions`
- `runtime_escape_air_frame212_p2_lands_when_decomp_floor_sweep_crosses_platform`
- `runtime_guard_neutral_stick_does_not_play_guard_direction_table_like_ftco_80091e78`
- `runtime_left_facing_catch_wait_holds_victim_on_faced_capturedamage_joint`
- `runtime_replay_nair_frame2253_source_collision_hits_dashing_victim`
- `runtime_replay_nair_frame2272_does_not_reuse_stale_damage_hurt_root_for_active_hitbox`
- `runtime_source_damage_fly_recent_lr_uses_baked_passive_binding`
- `runtime_source_damage_fly_roll_floor_contact_uses_baked_pose_to_enter_down_bound`
- `runtime_source_damage_high_knockback_uses_baked_hip_pose_to_enter_down_bound`
- `runtime_source_damage_stages_carry_decomp_damage_stage_fields`
- `runtime_source_hit_confirms_carry_decomp_hitbox_attributes`
- `sdl_runtime_launcher_uses_local_sdl_play_mode`
- `sdl_runtime_vanilla_launcher_disables_ucf_for_controller_testing`
- `slippi_first_divergence_reports_first_non_witness_divergence`
- `slippi_match_start_comparison_carries_first_non_witness_divergence`
- `slippi_match_start_frame2313_thrown_hi_keeps_decomp_accessory_position`
- `slippi_match_start_frame390_p1_dash_position_drift_cascades_from_p2_unresolved_root`
- `slippi_match_start_frame_negative47_p1_landing_velocity_drift_cascades_from_floor_commit_witness`
- `slippi_match_start_frame_negative48_p1_fall_platform_commit_is_mixed_phase_witness`
- `source_collision_frame_samples_catch_hitboxes_from_live_source_anim_frame`
- `source_collision_frame_samples_hitbox_script_capsules_from_source_pose_frame`
- `special_air_hi_hit_collision_uses_melee_transn_reset`
- `special_air_hi_hit_pills_sample_live_source_anim_frame`
- `visual_replay_gate_skips_proven_telemetry_and_pauses_on_first_state_mismatch`
- `wup_input_mapper_feeds_console_origin_pads_through_melee_processor`

The unrelated worktree change in `crates/mole_runtime/src/slippi_diagnostic.rs` was not modified or staged.
