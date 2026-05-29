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
- Jump/air/landing: `KneeBend`, `JumpF`, `JumpB`, `Air`, `JumpAerialF`,
  `JumpAerialB`, `EscapeAir`, `FallSpecial`, `Landing`, `LandingFallSpecial`
- Shield/defense: `GuardOn`, `Guard`, `GuardOff`, `EscapeN`, `EscapeF`,
  `EscapeB`
- Grab: `Catch`, `CatchDash`
- Ground attacks: `Attack1`, `AttackDash`, `AttackS3`, `AttackHi3`,
  `AttackLw3`, `AttackS4`, `AttackHi4`, `AttackLw4`
- Aerial attacks: `AttackAirN`, `AttackAirF`, `AttackAirB`, `AttackAirHi`,
  `AttackAirLw`
- Specials: `SpecialN`, `SpecialS`, `SpecialHi`, `SpecialLw`,
  `SpecialAirN`, `SpecialAirS`, `SpecialAirHi`, `SpecialAirLw`

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
| `WalkSlow/Middle/Fast` | One `walking` state plus `walkSlow`, `walkMiddle`, `walkFast` flags | `WalkSlow`, `WalkMiddle`, `WalkFast` | Rust now owns explicit walk state identity from rollback-owned input facts. Bucket thresholds are centralized in `MeleeCommonData`, source-mapped to `x28`, `x2C`, and `x30`, and covered by extractor/input-fact tests while exact retail data remains a future `PlCo.dat` extraction step. Walk acceleration, target speed, and friction now come from deterministic `FighterProfile` data instead of local simulation constants. |
| `Turn` | `turning` | `Turn` | Covered conceptually. Pygame mixes dash-out logic inside `turn()`. Rust is the cleaner model. |
| `TurnRun` | `runTurn` | `TurnRun` | Covered conceptually. Needs continued parity checks for old-facing acceleration and no-interrupt windows. |
| `Dash` | `dashing` | `Dash` | Covered. Current playable Pygame still recomputes dash taps from floats instead of consuming Rust's input facts. |
| `Run` | `running` | `Run` | Covered. |
| `RunDirect` | Missing | Missing | Low priority, but should be represented if we model run entry variants exactly. |
| `RunBrake` | `endLag` flag while running | `RunBrake` | Rust is closer. Pygame should stop using generic `endLag` for this. |
| `KneeBend` | `jumpSquat` | `KneeBend` | Covered conceptually. |
| `JumpF/JumpB` | Collapsed into `air` after jumpsquat | `JumpF`, `JumpB` | Rust is closer. Pygame cannot distinguish ground jump direction as a state. |
| `JumpAerialF/B` | Collapsed into `air` | `JumpAerialF`, `JumpAerialB` | Rust is closer. |
| `Fall/FallF/FallB/FallAerial/FallAerialF/FallAerialB` | Mostly `air` | Mostly `Air` | Missing explicit fall variants. This affects animation, aerial drift state identity, and collision transitions. |
| `FallSpecial/F/B` | `fallSpecial` | `FallSpecial` | Base state covered. Forward/back variants missing. |
| `Squat/SquatWait/SquatRv` | `crouchStart`, `crouching`; no explicit crouch release | `Squat`, `SquatWait`, `SquatRv` | Rust is closer. Crouch release now uses common-data `x94` hysteresis and profile-owned crouch startup/release durations; exact animation data remains a future extraction step. |
| `Landing` | `landingLag` | `Landing` | Rust now has a general landing state for ordinary airborne contact. Held shield does not skip landing lag; after landing lag finishes, normal grounded shield entry can happen on the next actionable frame. |
| `LandingFallSpecial` | `landingFallSpecial` | `LandingFallSpecial` | Covered conceptually. |
| Ground attacks | One jab, one tilt per direction, one smash per direction | Basic jab, dash attack, tilt/smash direction buckets | Missing jab chain/rapid jab and angled side tilt/smash variants. |
| Aerial attacks | Five aerial states | Five aerial states | Covered conceptually. Missing `LandingAir*` states in both. |
| `GuardOn/Guard/GuardOff` | `blocking` and `guardOff`; no real `GuardOn` | `GuardOn`, `Guard`, `GuardOff` | Rust is closer. Guard startup and release durations are now profile-owned action-frame values; Pygame shield behavior should migrate to Rust or mirror these exact boundaries. |
| `GuardSetOff/GuardReflect` | Missing | Missing | Needed for shield hitstun/powershield parity later. |
| `EscapeN/F/B/Air` | `dodge`, `roll`, `airDodge` umbrella states | `EscapeN`, `EscapeF`, `EscapeB`, `EscapeAir` | Rust is closer. Spotdodge and roll durations are now profile-owned action-frame values; root-motion-backed roll distance remains future parity work. |
| Damage/tumble | `hitstun` only | Missing | Huge missing area. Not urgent for empty movement, but essential for combat. |
| Knockdown/tech/passive | Missing | Missing | Essential later for floor interactions and competitive feel. |
| Shield break | Missing | Missing | Later combat completeness. |
| Catch/throw/captured/thrown | No real throw state | `Catch`, `CatchDash` only | Needs catch pull/wait/attack/cut and throw/thrown families later. |
| Ledge/cliff | Missing | Missing | Not required for flat-stage movement, required for platform fighter completeness. |
| Item states | Missing | Missing | Not needed unless items are a design goal. |
| Environment/object states | Missing | Missing | Mostly optional unless we add equivalents. |

## Logic Pass Notes

The input path is currently conceptually clean at the native boundary:

1. Rust WUP reads raw GameCube bytes and buttons.
2. Rust captures a console-style origin on connect.
3. Rust applies the UCF-style input pass and produces Melee-shaped facts.
4. Python receives normalized axes and selected facts through `NativeWupInput`.

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
- Rust input processing derives Melee/UCF facts.
- The motion-state interpreter consumes those facts.
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

1. Add `Fall`, `FallF`, `FallB`, `FallAerial`, `FallAerialF`,
   `FallAerialB`.
2. Add `LandingAirN`, `LandingAirF`, `LandingAirB`, `LandingAirHi`,
   `LandingAirLw`.
3. Add `GuardSetOff` and `GuardReflect` once shield hit behavior is implemented.
4. Add tech/knockdown/passive states before building full combat.
