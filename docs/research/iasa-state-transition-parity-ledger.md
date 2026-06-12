# IASA And State Transition Parity Ledger

Updated: 2026-06-12

Purpose: track Melee common-state callback coverage in the shape the decomp uses:
`Anim -> IASA -> Phys -> Coll`, plus any transition helpers those callbacks
install. This is an index, not source truth. Re-check the decomp before changing
runtime behavior.

Primary source anchors:
- `.research/doldecomp-melee/src/melee/ft/ftmotionstates.c`
- `.research/doldecomp-melee/src/melee/ft/chara/ftCommon/*`
- `.research/doldecomp-melee/src/melee/ft/ft_0D4D.c`
- `.research/doldecomp-melee/src/melee/ft/ftcliffcommon.c`
- `.research/doldecomp-melee/src/melee/mp/mpcoll.c`

Rules:
- Every common motion-state family should eventually have one row per source
  state or state family.
- Each row records the decomp callbacks, Rust owner, tests, and remaining gaps.
- If Rust intentionally collapses callback order for performance, the row must
  explain why it is still source-equivalent and rollback-safe.
- Slippi replay interop is a validation contract. Slippi tells us what replay
  data must be consumed; the decomp tells us how simulation state advances.

## Ledger

| State family | Source IDs | Decomp callbacks | Rust status | Tests | Remaining work |
| --- | ---: | --- | --- | --- | --- |
| Entry / EntryStart / EntryEnd | 322-324 | `ftmotionstates.c`; Entry files neighboring `ft_0D4D.c`; accessory from `Fighter_804D6514` | Partial. Rust has staggered Entry, source-backed platform cue, baked platform bounds/wireframe, and rollback match phase metadata. | `render_entry_platform_lifecycle_matches_source_accessory_states`; `render_entry_platform_exposes_source_bounds_wireframe`; `render_scene_exposes_match_intro_and_entry_platform_cues` | Confirm exact Entry callback file/function mapping and full accessory lifetime. |
| Rebirth / RebirthWait | 12-13 | `ft_0D4D.c`: `ftCo_Rebirth_*`, `ftCo_RebirthWait_*`, `ftCo_800D5600`, `fn_800D54A4`, `fn_800D55B4` | Partial. Rust has stock-loss to Rebirth/RebirthWait, source `x5D8` intangibility on timed exit, hard-down RebirthWait IASA exit to Fall, and rendered source platform cue/wireframe during RebirthWait. | `rebirth_wait_exit_installs_source_hurt_intangibility_from_x5d8_before_fall`; `rebirth_wait_down_input_exits_spawn_platform_into_fall_with_source_intangibility`; `render_respawn_wait_exposes_source_platform_cue` | Broaden RebirthWait IASA coverage for jump/special/attack/turn/walk helpers after each helper is checked against source. Render source-owned intangibility/invincibility as wireframe tint only after checking the decomp visual/effect path. |
| Pass / platform drop-through | 244 | `mpcoll.c` platform pass callbacks; common state `Pass` | Partial. Rust has common-data driven pass thresholds, delay, `Pass` state, floor-skip surface tracking, and soft-platform rejection tests. | `shield_down_on_soft_platform_enters_pass_not_custom_drop_state`; `squat_platform_pass_delay_enters_shared_pass_state_without_shield`; `pass_entry_records_and_clears_source_floor_skip` | Reconcile with RebirthWait platform release and moving-platform callback edge cases. |
| Cliff / ledge | 252-263 family in source action data | `ftcliffcommon.c`; `ftCo_CliffWait.c`; `ftCo_CliffClimb.c`; `mpcoll.c`; stage ledges from extracted collision | Partial. Battlefield ledges are promoted into `StageProfile`; airborne fighters can enter source `CliffCatch`/252 using extracted ledge points, Falcon source snap offsets, source-facing side rules, hard-down block, rollback-owned ledge id, and generated source ECB samples. `CliffCatch_Anim` transitions to `CliffWait`/253 at the source animation duration while cliff phys pins to the same ledge. Occupied ledges reject new grabs, `x2064_ledgeCooldown` is rollback-owned, and `CliffWait` uses extracted `PlCo.dat` fields `x488`/`x48C`/`x490`/`x494`/`ledge_cooldown`/`x49C` for timer, option gate, cooldown, and hurt intangibility. Away/down release to Fall is implemented after the source `mv.co.cliff.x8`-style neutral gate. | `airborne_fighter_near_battlefield_ledge_enters_source_cliff_catch`; `source_cliff_catch_animation_end_enters_cliff_wait_on_same_ledge`; `source_cliff_catch_rejects_ledge_occupied_by_another_fighter`; `source_ledge_cooldown_blocks_immediate_regrab_and_ticks_down`; `source_cliff_wait_uses_plco_timer_and_hurt_intangibility`; `source_cliff_wait_arms_gate_then_away_down_releases_to_fall_with_cooldown`; `rust_motion_states_all_have_source_ecb_samples`; stage extraction tests assert Battlefield ledge points. | Implement `CliffAttack*`, `CliffEscape*`, `CliffJump*`, and `CliffClimb*` option states, plus deeper cliff collision callbacks from `ftcliffcommon.c`/`mpcoll.c`. Current away/down release is the only CliffWait option behavior translated. |
| Damage / DamageFly / DownBound / Passive | 75-91, 183-204 | `ftCo_Damage.c`, `ftCo_DownBound.c`, `ftCo_Passive*.c` | Partial. Source damage/hitlag/hitstun/downbound/passive floor slices exist. Launch now stores decomp-style source knockback velocity separately from self velocity, and first hitlag callbacks translate `OnEveryHitlag` SDI plus `OnExitHitlag` ASDI/DI/LR knockback scaling from PlCo fields. | `source_damage_application_stores_decomp_kb_velocity_separately_from_self_velocity`; `source_damage_hitlag_applies_sdi_from_plco_window_like_ftco_damage_every_hitlag`; `source_damage_exit_hitlag_applies_asdi_and_di_to_source_kb_velocity`; existing DamageFly/Passive/DownBound tests in `core_contract.rs` | Tumble, wall/ceiling passive, and full tech branches remain. |

## Update Protocol

- Add a row when a new common state family becomes active work.
- Add exact decomp function names before implementing.
- Add the test names that prove the current Rust status.
- Move vague "partial" notes into concrete gaps as soon as source evidence is
  known.
