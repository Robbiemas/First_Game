# Contract Failure Classification - 2026-07-15

This inventory is the implementation-order authority after the strict
Falcon/Battlefield replay checkpoint. A listed failure is not permission to
change production behavior until its decomp owner has been inspected.

## Baseline

- `mole_core --test core_contract`: 568 passed, 46 failed, 2 ignored.
- `mole_runtime --test runtime_contract`: 276 passed, 31 failed, 0 ignored.
- Runtime parity contracts use the tracked compressed 5,313-frame fixture; no
  failure is hidden by a missing local debug export.

## Core Contracts

Twenty-eight failures are stale fixtures or expectations:

- Legacy profile/action descriptor assumptions (10):
  `attack_dash_total_duration_uses_profile_action_frames`,
  `dash_neutral_and_run_brake_use_x60_ground_friction_not_generic_high_speed_traction`,
  `falcon_squat_rv_uses_extracted_action_animation_length`,
  `run_acceleration_uses_source_x5c_remaining_velocity_taper`,
  `run_brake_cmd_var0_window_can_branch_to_turnrun_before_wait_or_walk`,
  `run_x430_decrements_before_iasa_allows_turnrun_on_boundary_frame`,
  `standing_turn_delays_facing_flip_until_profile_flip_frame`, and the three
  `turn_run_*` completion contracts.
- Incomplete source-pose/CollData setup (10): the two `escape_air_landing_*`,
  `ground_jump_escape_air_*`, two `ordinary_airborne_contact_*`, two
  `slippi_*landing*`, and three `source_damage_*floor_contact*` contracts.
- Old generated-pose/ECB aliases (5): `aerial_jump_from_pass_*`,
  `render_snapshot_exposes_generated_fall_special_forward_*`, and the three
  directional-submotion render contracts.
- Incomplete private-state setup (3): `aerial_lcancel_*`,
  `guard_reflect_jump_squat_entry_*`, and `source_catch_attack_anim_end_*`.

Eighteen failures are production architecture gaps:

- Shared AObj/FObj and fighter callback scheduler (7):
  `aerial_attack_iasa_advances_new_action_anim_on_entry_tick`,
  `attack100_start_loop_and_end_follow_decomp_action_chain`,
  `dash_neutral_falls_back_on_animation_completion_not_profile_dash_frames`,
  `ground_jump_takeoff_carries_moonwalk_followthrough_slide_without_custom_state`,
  `neutral_special_latched_during_turn_replays_with_current_stick_on_turn_frame`,
  `rebirth_wait_timer_expiry_enters_fall_before_same_tick_fall_physics`, and
  `source_damage_fly_anim_exit_enters_damage_fall_after_lockout_clears`.
  Source owners: `Fighter_procUpdate`, `ftAnim_8006EBA4`,
  `ftAnim_IsFramesRemaining`, and `HSD_AObjInterpretAnim`.
- Shield/lightshield lifecycle and hit processing (4): `analog_lightshield_*`,
  `shield_cstick_down_*`, `source_guard_shield_confirm_*`, and
  `source_shield_confirm_drains_*`. Source owners: `ftCo_800921DC`, Guard
  object lifecycle, and `Fighter_ProcessHit_8006D1EC`.
- Hitlag callback/input phase (1): `source_damage_hitlag_applies_sdi_*`.
  Source owners: `Fighter_8006A1BC` and `ftCo_Damage_OnEveryHitlag`.
- Missing source action-pose/ECB coverage (1):
  `rust_motion_states_all_have_source_ecb_pose_data`; missing states are
  DamageFall, six ShieldBreak states, and Furafura.
- Capture/throw live-JObj accessories (5): two `source_capture_pulled_lw_*`,
  two `source_throw_hi_b3_release_*`, and `source_thrown_hi_uses_*`.
  Source owners: `ftCo_CapturePulledLw_Phys`, `lb_8000B1CC`,
  `ftCo_800DD724`, `ftCo_800DDDE4`, and `ftCo_800DE508`.

## Runtime Contracts

Two baseline failures were stale launcher expectations and are now reconciled with the
release-runtime launcher contract:

- `sdl_runtime_launcher_uses_local_sdl_play_mode`
- `sdl_runtime_vanilla_launcher_disables_ucf_for_controller_testing`

The remaining 29 failures are grouped by shared production owner:

- Source-frame render selection (6): `attack100_loop_hit_pills_*`,
  `looping_locomotion_source_capsules_*`, three `render_source_selection_*`
  contracts, and `special_air_hi_hit_pills_*`.
- Collision, hit confirmation, and damage staging (8):
  `frame_debug_log_reports_*`, two `runtime_replay_nair_*`,
  `runtime_source_damage_stages_*`, `runtime_source_hit_confirms_*`, two
  `source_collision_frame_samples_*`, and `special_air_hi_hit_collision_*`.
- Core motion, grounding, capture, and damage state (9):
  `runtime_escape_air_frame212_*`, `runtime_guard_neutral_stick_*`,
  `runtime_left_facing_catch_wait_*`, three `runtime_source_damage_*`, and the
  three `slippi_match_start_frame_negative47/negative48/390_*` contracts.
- Throw/capture accessory transform (1):
  `slippi_match_start_frame2313_thrown_hi_keeps_decomp_accessory_position`.
- Shield rendering (1):
  `render_scene_draws_translucent_bubble_shield_for_guard_states`.
- Slippi comparison/strict divergence gate (3):
  `slippi_first_divergence_reports_first_non_witness_divergence`,
  `slippi_match_start_comparison_carries_first_non_witness_divergence`, and
  `visual_replay_gate_skips_proven_telemetry_and_pauses_on_first_state_mismatch`.
- WUP input translation (1):
  `wup_input_mapper_feeds_console_origin_pads_through_melee_processor`.

## Dependency Order

1. Shared AObj/FObj playback and `Fighter_procUpdate` callback scheduler.
2. Missing source pose/ECB extraction.
3. Shared collision, hitlag, shield, and damage processing.
4. Capture/throw accessory transforms over live JObj output.
5. Runtime render/collision selection consumers.
6. Stale fixture updates after each corresponding production path is proven.
7. Diagnostic comparison expectations.

Every group must retain the strict replay gate. No failure may be removed by
relaxing comparison tolerances, allowing replay continuation, or introducing a
fixture-specific gameplay branch.
