# Mole State Coverage Compared To Melee Common States

This compares the current Mole prototype against the Melee common fighter state
inventory in [melee-common-state-inventory.md](./melee-common-state-inventory.md).

There are currently two state surfaces:

- The playable Pygame prototype in `Characters.py` and `ChooseAction.py`.
- The newer deterministic Rust core in `crates/mole_core/src/state.rs` and
  `crates/mole_core/src/sim.rs`.

The Rust core is the cleaner long-term direction. The Pygame prototype is the
current hands-on testing surface.

## Current Pygame States

Defined by `Character.d` in `Characters.py`:

- Movement: `standing`, `walking`, `running`, `dashing`, `turning`,
  `runTurn`, `endDash`
- Air/jump/landing: `air`, `jumpSquat`, `landingLag`,
  `landingFallSpecial`, `freeFall`, `fallSpecial`, `airDodge`
- Crouch: `crouchStart`, `crouching`
- Shield/defense: `blocking`, `guardOff`, `shieldstun`, `dodge`, `roll`
- Damage: `hitstun`
- Ground attacks: `jab`, `ftilt`, `utilt`, `dtilt`, `fsmash`, `usmash`,
  `dsmash`
- Aerial attacks: `nair`, `fair`, `bair`, `uair`, `dair`
- Specials: `special`, `uspecial`, `dspecial`, `fspecial`

Important note: several Pygame states are legacy umbrella states. For example,
`walking` holds slow/middle/fast as flags, `air` covers multiple Melee fall and
jump states, and `blocking` represents steady guard while `guardOff` is the only
explicit shield-exit state.

## Current Rust Core States

Defined by `MotionState` in `crates/mole_core/src/state.rs`:

- Movement: `Wait`, `WalkSlow`, `WalkMiddle`, `WalkFast`, `Dash`, `Run`,
  `RunBrake`, `TurnRun`, `Turn`
- Crouch: `Squat`, `SquatWait`, `SquatRv`
- Jump/air/landing: `KneeBend`, `JumpF`, `JumpB`, `Fall`, `JumpAerialF`,
  `JumpAerialB`, `EscapeAir`, `FallSpecial`, `Landing`, `LandingFallSpecial`,
  `Pass`
- Shield/defense: `GuardOn`, `Guard`, `GuardOff`, `EscapeN`, `EscapeF`,
  `EscapeB`
- Grab: `Catch`, `CatchDash`
- Ground attacks: `Attack1`, `AttackDash`, `AttackS3`, `AttackHi3`,
  `AttackLw3`, `AttackS4`, `AttackHi4`, `AttackLw4`
- Aerial attacks: `AttackAirN`, `AttackAirF`, `AttackAirB`, `AttackAirHi`,
  `AttackAirLw`
- Specials: `SpecialN`, `SpecialSStart`, `SpecialS`, `SpecialHi`, `SpecialLw`,
  `SpecialAirN`, `SpecialAirSStart`, `SpecialAirS`, `SpecialAirHi`,
  `SpecialAirLw`

Important note: Rust has fewer states than Melee, but its state boundaries are
closer to Melee than Pygame. It already separates `GuardOn`, `Guard`,
`GuardOff`, `EscapeAir`, `FallSpecial`, `Landing`, `LandingFallSpecial`, `RunBrake`,
`TurnRun`, explicit walk buckets, crouch hold/release states, and
ground-vs-air specials.

## Coverage Matrix

| Melee common area | Pygame coverage | Rust coverage | Gap / direction |
| --- | --- | --- | --- |
| Death/rebirth/entry | Missing | Missing | Not needed for movement feel yet, but required for full match flow. |
| `Wait` | `standing` | `Wait` | Covered conceptually. Rust name should be canonical. |
| `WalkSlow/Middle/Fast` | One `walking` state plus `walkSlow`, `walkMiddle`, `walkFast` flags | `WalkSlow`, `WalkMiddle`, `WalkFast` | Rust now owns explicit walk state identity from rollback-owned input facts. Walk entry uses extracted `PlCo.dat` `x24 == 23`; slow/middle/fast buckets are chosen from current `gr_vel` against decomp-backed `x28`/`x2C`, and walk acceleration, target speed, taper `x30`, friction, and `mv.co.walk.x0 = target_vel * x440` storage follow `ftWalkCommon_800E0060` from deterministic Falcon profile/common data. Walk animation rate and bucket remap now carry source-shaped `mv.co.walk.x0`/rate fields, while exact per-bucket source animation-frame seeds remain future extraction work. Walk exit through the `ft_8008A244`-shaped path preserves carried `gr_vel` through the state change; because Melee input callbacks run before the later physics priority, the resulting `Wait` can still apply same-frame `Wait_Phys` friction before that carry feeds opposite-dash moonwalk setup. |
| `Turn` | `turning` | `Turn` | Rust preserves carried `gr_vel`, applies source-shaped ground friction instead of zeroing velocity, delays facing flip through the profile frame count, models the source temporary-facing action check for pre-flip attacks, carries A/B latch fields, and traces the hidden Turn vars for controller-log diagnosis. Full source helper identity and edge collision are still broader parity work. |
| `TurnRun` | `runTurn` | `TurnRun` | Rust models the extracted PlCo `x38 == -48` Run-to-TurnRun threshold, old-facing `accel_mul`, same-frame source-shaped TurnRun physics after Run IASA entry, the Falcon action-script `cmd_var[1]` pause, delayed facing flip at the velocity-crossing resume gate, jump-only IASA, completion-gated `fn_800CA644` Run handoff, and the extracted `x430` no-interrupt handoff back into Run. Broader collision/projection parity remains partial. |
| `Dash` | `dashing` | `Dash` | Rust owns the decomp-shaped dash/tap core with extracted `PlCo.dat` deadzones (`x0`/`x4 == 36`), tap thresholds (`x8`/`xC == 32`), dash threshold (`x3C == 102`), tap window (`x40 == 2`), IASA windows (`x44`/`x48`/`x4C == 4/3/20`), `x54` Dash IASA velocity decay, source-shaped dash-entry velocity delta, first-frame `mv.co.dash.x0` suppression of ordinary Dash_Phys acceleration, tap-start early-Dash gating, Falcon Dash action-script `cmd_var[0]` frame-16 Run gate, same-frame Run_Phys handoff after `fn_800CA5F0`, figatree frame-29 non-run fallback, Run-to-TurnRun threshold (`x38 == -48`), and run threshold (`x58 == 79`). Grounded horizontal movement now tracks decomp-shaped `gr_vel`, same-frame `xE4_ground_accel_1`, and post-translation Dash-entry `xE8_ground_accel_2` equivalents instead of directly overwriting horizontal velocity. Moonwalk-like movement remains an emergent Dash/tap-timer/live-stick outcome, including the bottom-gate payload `(+127,0) -> (-101,-45) -> (-101,-45) -> (-128,0)` and chained relay tests before Falcon's initial dash animation resolves. Non-run Dash completion now falls back to `Wait` with carried `gr_vel`; held-back follow-through is then handled by Wait's normal Turn/Walk priority instead of a direct Dash-to-Walk shortcut. |
| `Run` | `running` | `Run` | Covered. |
| `RunDirect` | Missing | `RunDirect` | Rust now has the explicit state identity, Slippi common-action mapping (`22`), checksum ID, legacy running visual cue, and Captain Falcon ECB mapping through decomp `ftCo_SM_Run` / action-table slot 13. The visible `ftCo_RunDirect_IASA` same-facing run handoff into `Run` and release fallback to `Wait` are covered for injected/diagnostic RunDirect state identity. A normal gameplay entry into `RunDirect` was not found in the local decomp search, so this remains partial rather than a reachable movement branch. |
| `RunBrake` | `endLag` flag while running | `RunBrake` | Rust has explicit RunBrake identity, source-shaped `gr_friction * x60` braking, jump/squat interrupts, extracted Captain Falcon `max_run_brake_frames == 30`, Falcon action-script command-var timing, PlCo `x42C` animation pause behavior, and the command-var-gated TurnRun branch. Pygame should stop using generic `endLag` for this. |
| `KneeBend` | `jumpSquat` | `KneeBend` | Rust has Falcon 4f jumpsquat, short-hop source tracking, and ECB coverage mapped through exact decomp submotion `ftCo_SM_Kneebend` / action-table slot 15. Grounded entries now mirror the source callback order by applying `ftCo_KneeBend_Phys`/`ft_80084F3C` friction on the entry frame; takeoff does not apply an extra KneeBend friction tick. Captain Falcon's DAT points that slot at the Landing figatree bytes, so this is recorded as a source-table mapping, not a visual alias. |
| `JumpF/JumpB` | Collapsed into `air` after jumpsquat | `JumpF`, `JumpB` | Rust is closer. Pygame cannot distinguish ground jump direction as a state. Profile-owned horizontal jump scaling now treats HSD-clamped native stick `+127` as positive Melee `1.0` and `-127` as negative Melee `-1.0`; raw adapter `-128` is preserved for diagnostics before the HSD/fighter clamp. Ground jump carry is now tested through the source `self_vel`/`gr_vel` handoff for walk, dash, Wait slide, and moonwalk follow-through: carried grounded speed receives jumpsquat friction through the prior grounded frame, is scaled by `ground_to_air_jump_momentum_multiplier`, and combines with held-stick `jump_h_initial_velocity` before the `jump_h_max_velocity` clamp. Rust also uses generated Captain Falcon action-frame counts to route `JumpF/B` animation completion into base `Fall`, matching `ftCo_Jump_Anim`. |
| `JumpAerialF/B` | Collapsed into `air` | `JumpAerialF`, `JumpAerialB` | Rust is closer. Profile-owned air-jump horizontal velocity uses the same HSD-clamped `/127.0f` fighter stick scale. The decomp also routes aerial-jump horizontal velocity through `init_h_vel`, `x74_anim_vel`, and `self_vel`, so full aerial parity is still source-audit work. |
| `Fall/FallF/FallB/FallAerial/FallAerialF/FallAerialB` | Mostly `air` | Explicit `Fall` and `FallAerial`; Fall family ECB mapped | Rust removed the generic `Air` umbrella. Generic airborne fall-through routes to `Fall`, matching `ftCo_Fall_Enter`; completed aerial-jump animation routes to `FallAerial`, matching `ftCo_FallAerial_Enter`. `FallF/B` and `FallAerialF/B` now have exact action-table ECB samples, and base `Fall`/`FallAerial` select the active directional ECB/render pose from the `ftCo_Fall_Anim_Inner` velocity threshold using PlCo `x444`/`x448` without changing gameplay state. Full ftAnim blend metadata remains pending. |
| `FallSpecial/F/B` | `fallSpecial` | `FallSpecial`, `FallSpecialF`, `FallSpecialB` | Base state is entered from EscapeAir and other special-fall paths. Forward/back variants now exist as exact Rust states with generated action-table ECB samples, and base `FallSpecial` selects the active directional ECB/render pose through the same `ftCo_Fall_Anim_Inner`-shaped velocity threshold. Full special-fall animation blend metadata and remaining mobility entry paths remain pending. |
| `Squat/SquatWait/SquatRv` | `crouchStart`, `crouching`; no explicit crouch release | `Squat`, `SquatWait`, `SquatRv` | Rust is closer. Crouch release now uses common-data `x94` hysteresis and profile-owned crouch startup/release durations; exact animation data remains a future extraction step. |
| `Landing` | `landingLag` | `Landing` | Rust now has a general landing state for ordinary airborne contact. `ftCo_Landing_IASA`-style interrupts are gated by normal landing lag, while neutral/no-input completion exits through the separate `ftCo_Landing_Anim -> ft_8008A2BC -> Wait` no-frames path. Held shield does not skip landing lag or trap the fighter in Landing. |
| `LandingFallSpecial` | `landingFallSpecial` | `LandingFallSpecial` | Rust preserves the air-dodge/special-fall slide, applies source-shaped ground traction, exits on `x344` landing lag, and now samples the shared Landing figatree at the source-scaled LandingFallSpecial animation rate for active ECB/render pose. Non-airdodge entry paths with different landing lag or interrupt flags remain partial. |
| `Pass` | Missing | `Pass` | Rust models the shared platform drop-through state for shield-held and Squat countdown entries without a custom shield-drop state. Entry uses common-data `x46C` vertical velocity, enters the `ftCommon_8007D5D4`-style ECB bottom lock as a floor-contact probe, physics follows `ftCo_Pass_Phys -> ft_80084DB0`, generated Falcon action 209 ECB samples take over after lock expiry, and generated Falcon action 209 completion now routes to `Fall` through the `ftCo_Pass_Anim` boundary. Character-special routes that call `ftCo_8009A184` remain future per-character work. |
| Ground attacks | One jab, one tilt per direction, one smash per direction | Basic jab, dash attack, tilt/smash direction buckets | Missing jab chain/rapid jab and angled side tilt/smash variants. |
| Aerial attacks | Five aerial states | Five aerial states plus `LandingAirN/F/B/Hi/Lw` | Aerial attack entry states are covered conceptually. Rust now enters exact `LandingAir*` states from aerial-attack ground contact, maps their generated Falcon ECB samples, keeps `ftCo_LandingAir_IASA` empty, and exits by the shared `ftCo_Landing_Anim` no-frames path using the extracted `landingair*_lag` window. Stale landing-lag scaling and render-pose animation-rate sampling from `ftCo_LandingAir_EnterWithLag` remain pending. |
| `GuardOn/Guard/GuardOff` | `blocking` and `guardOff`; no real `GuardOn` | `GuardOn`, `Guard`, `GuardOff` | Rust is closer. Guard startup and release durations are now profile-owned action-frame values; Pygame shield behavior should migrate to Rust or mirror these exact boundaries. |
| `GuardSetOff/GuardReflect` | Missing | `GuardReflect` partial, `GuardSetOff` partial | Rust now implements the Dash fresh digital L/R `GuardReflect` entry slice using extracted `x2A0` and preserves the Dash IASA `x54` velocity decay. ECB coverage maps GuardReflect through exact decomp submotion `ftCo_SM_GuardOn` / action-table slot 37. GuardSetOff now exists as a Rust state identity with Slippi action `181` and ECB coverage through `ftCo_SM_GuardDamage` / action-table slot 40; shield-hit entry math, hitlag callbacks, SDI, and animation exit routing remain future hitbox/shield-system work. |
| `EscapeN/F/B/Air` | `dodge`, `roll`, `airDodge` umbrella states | `EscapeN`, `EscapeF`, `EscapeB`, `EscapeAir` | Rust is closer. Spotdodge and roll durations are now profile-owned action-frame values. `EscapeAir` uses source common-data deadzone/force/decay/landing lag, preserves any live ground-to-air ECB floor-contact lock, samples generated Falcon action 44 ECB after lock expiry, and exits through the generated Falcon action 44 sample count instead of the legacy common-data duration placeholder. Root-motion-backed roll distance remains future parity work. |
| Damage/tumble | `hitstun` only | `SourceDamage` partial | Common Damage/DamageFly ids 75-91 now enter as canonical source-only action states with hitlag freeze, `damage_hitstun_frames`, baked `source_action_total_frames`, source hurtbox/render/collision pose ownership, and the locked/unlocked `ft_80084EEC` vs `ft_80084DB0` air-physics split. Ordinary Damage ids 75-86 now mirror the implemented `ftCo_Damage_Coll` floor-contact slice using extracted CommonAttributes `x1E0 = 5.0` and `x1E4 = 0.5`: medium knockback enters basic `Landing`, high knockback enters DownBoundU/D action ids 183/191 using the baked FtPart_HipN orientation check. DamageFly/DamageFlyRoll floor contact now mirrors the represented decomp order: PassiveStand then Passive through `ftCo_800986B0`, backed by rollback-owned `x680`/`x684` digital L/R timers and extracted CommonAttributes `x1C`, `x250`, and `x254`; failed passive checks fall through to DownBoundU/D through the same baked hip-pose gate. DownBound animation end enters DownWaitU/D 184/192, DownWait timer expiry enters DownStandU/D 186/194, fresh source-normalized A/B enters DownAttackU/D 187/195, and fresh source-normalized LR enters DownStandU/D through baked source action data. SDI/ASDI, tumble branches, Hammer-item passive veto state, wall/ceiling damage callbacks, DownWait side getup/roll routing, vertical-stick stand-up thresholds, exact PassiveStand model-velocity physics, and downed damage/passive callbacks remain pending. |
| Knockdown/tech/passive | Missing | `SourcePassive` partial | Ground Passive/PassiveStand action ids 199-201 are now baked source-only runtime actions and reachable from DamageFly/DamageFlyRoll floor contact. Broader knockdown teching, wall/ceiling passive states, item vetoes, and downed passive callbacks remain later parity work. |
| Shield break | Missing | Missing | Later combat completeness. |
| Catch/throw/captured/thrown | No real throw state | `Catch`, `CatchDash` only | Needs catch pull/wait/attack/cut and throw/thrown families later. |
| Ledge/cliff | Missing | Missing | Not required for flat-stage movement, required for platform fighter completeness. |
| Item states | Missing | Missing | Not needed unless items are a design goal. |
| Environment/object states | Missing | Missing | Mostly optional unless we add equivalents. |

## Logic Pass Notes

The input path is currently conceptually clean at the native boundary:

1. Rust WUP reads raw GameCube bytes and buttons.
2. Rust captures a console-style origin on connect.
3. Rust derives a native `PADRead`/HSD-shaped pad sample: origin-subtracted
   triggers, origin-subtracted sticks, and HSD radius-80 clamped stick vectors
   scaled into the current compact fighter-facing axis.
4. Rust applies the adapter-owned optional UCF-style input pass before core
   input.
5. The Rust core derives vanilla Melee-shaped facts from the preprocessed pad.

The state path is improving, but is still less clean than the input path:

1. Python keeps its legacy `xTapTimer` for keyboard and generic controller
   fallback.
2. Native WUP input now passes Rust `dash_direction` and `x_tap_timer` into
   `Character.has_fresh_x_tap()`.
3. The playable Pygame movement resolver still owns many movement transitions
   that the Rust core already models more cleanly.

That means the main remaining risk is no longer duplicated dash timing for the
native controller path. It is state-transition drift inside Pygame itself.

## Recommended Foundation Rule

For the rollback engine path, input interpretation should have one owner:

- `GameCubePadStatus` is the source packet.
- `mole_input`/WUP preprocessing owns optional UCF cleanup before the core.
- Rust core input processing derives vanilla Melee facts.
- The motion-state interpreter consumes those vanilla facts.
- Pygame should be treated as a temporary renderer/test shell, not a second
  movement engine.

Short-term, if we keep testing through Pygame, each movement fix should be a
small transition-parity correction against the Rust/decomp reference. Long-term,
the right path is to make the Rust `MotionState` interpreter authoritative and
render its snapshots.

See [pygame-movement-logic-pass.md](./pygame-movement-logic-pass.md) for the
current bottom-up Pygame movement audit notes.

## Priority State Add List

These are the next missing or collapsed states most likely to affect immediate
movement feel:

1. Finish the real ftAnim directional-fall blend accumulator/metadata path for
   `FallF/B`, `FallAerialF/B`, and `FallSpecialF/B`; active ECB/render pose
   selection is now source-shaped, but the animation system is still partial.
2. Finish `GuardReflect` shield-hit behavior and wire `GuardSetOff` entry/exit
   once shield hit behavior is implemented.
3. Continue tech/knockdown/passive parity beyond the implemented ground Passive entry slice before building full combat.
