# Movement And Shield Parity Design

Date: 2026-05-29

## Purpose

Bring the Rust-authoritative core to a feel-testable movement baseline before
building combat. The first priority is wavedash-critical behavior: ground to air
transition through jumpsquat, first-airborne air dodge, EscapeAir physics,
FallSpecial handoff, LandingFallSpecial, and collision/landing behavior that
lets those states feel coherent on the Battlefield-like stage.

Combat systems such as hitboxes, hurtboxes, damage, knockback, hitlag, shield
stun, grab boxes, and move callbacks are explicitly out of scope for this design.
Attacks may remain simple motion-state transitions until movement and defensive
feel are testable.

## Order Of Work

1. Wavedash-critical movement parity.
2. Ground, platform, and ECB collision needed for movement feel.
3. Defensive shield movement: GuardOn, Guard, GuardOff, rolls, spotdodge, shield
   jump, shield drop, and platform drop.
4. Data extraction seams for values still marked provisional.
5. Combat loop systems after movement and shield feel are validated.

## Wavedash-Critical Movement

The Rust core already has the required broad states: `KneeBend`, `JumpF`,
`JumpB`, `JumpAerialF`, `JumpAerialB`, `Air`, `EscapeAir`, `FallSpecial`,
`Landing`, and `LandingFallSpecial`. The next work should tighten their source
shape rather than add a new gameplay layer.

`KneeBend` must continue to block air dodge. A fresh digital L/R press should
only become `EscapeAir` after the fighter is airborne, including shield jump
cases. Z-lightshield synthesis must not count as a digital air-dodge press.

`EscapeAir` should own the air-dodge self-velocity vector, deadzone check,
decay, no-gravity action phase, IASA/action timer, and transition into
`FallSpecial`. Values already represented in `MeleeCommonData` should be used
from that data seam, with provenance recorded. Missing exact values should remain
explicitly provisional until a real `PlCo.dat` extraction source is available.

`LandingFallSpecial` should preserve horizontal slide, apply grounded traction,
block normal grounded actions until its lag expires, and return to `Wait` only
after the proper source-shaped lag. If floor contact is lost during landing lag,
the state should return to an airborne state instead of snapping to idle.

## Collision And Stage Contact

Movement parity needs more than vertical ground snap. The Rust core should own a
deterministic ECB-vs-stage contact path that handles the current diamond ECB,
main floor, soft platforms, airborne-to-ground contact, landing state selection,
and platform drop-through rules.

For this phase, collision should stay focused on feel-critical movement:
horizontal walls, ledges, cliff catch, ceiling collision, and full stage hazard
behavior can be added after basic ground/platform movement is reliable. The
design should still keep state fields deterministic and rollback-covered so
those contacts can be added without changing architecture.

## Shield And Defensive Movement

After wavedash-critical flow is stable, shield work should cover state-local
defensive behavior:

- `GuardOn` startup and transition into `Guard`.
- `Guard` hold/release, analog shield amount, digital L/R identity, and Z
  pseudo-shield behavior.
- `GuardOff` release lag and source-shaped restrictions.
- Spotdodge from down tap or C-stick down.
- Roll forward/back from horizontal tap or C-stick side.
- Jump out of shield through the same `KneeBend` path as normal grounded jump.
- Shield drop/platform drop from source-shaped down inputs and UCF shield-drop
  preprocessing.

Shield damage, shield stun, shield setoff, powershield, shield health, and hit
interaction are combat-adjacent and should wait until hitboxes/hurtboxes exist.

## Data Ownership

Gameplay values must live in Rust data seams, not Pygame and not host input code.
Use these owners:

- `MeleeCommonData` for shared thresholds, timer windows, escape-air data, roll
  and spotdodge thresholds, shield-drop/drop-through values, and other `PlCo.dat`
  common data.
- `FighterProfile` for Captain Falcon movement attributes.
- `FighterActionFrames` for action durations and IASA-like frame gates that
  eventually come from Falcon action/animation data.
- `StageProfile` for Battlefield-like surface and blast-zone units.

If exact Melee data is missing, the field must stay marked as provisional in
docs and tests. Do not tune hidden constants by feel.

## Testing Strategy

Rust tests are the authority. Each mechanic should be added with a failing test
first, then a minimal implementation, then full verification.

Required test themes:

- Air dodge cannot start during `KneeBend`.
- First-airborne fresh digital L/R enters `EscapeAir`.
- Z and analog shield do not trigger air dodge.
- Air dodge vector uses deadzone plus fixed-force direction.
- EscapeAir decay/no-gravity/action-phase behavior is deterministic.
- EscapeAir animation completion enters `FallSpecial`.
- Landing during `EscapeAir` or `FallSpecial` enters `LandingFallSpecial`.
- LandingFallSpecial preserves slide and applies traction until lag expires.
- Losing floor contact during landing lag returns to airborne state.
- Soft-platform contact, platform drop-through, shield drop, roll, and spotdodge
  use `MeleeCommonData` thresholds and rollback-owned input snapshots.

Verification before each commit should include focused Rust tests, then the
workspace/runtime verification gate already used on this branch.

## Runtime Feel Testing

Once wavedash-critical tests pass, the SDL3 runtime should expose the result in
the existing visual layer with Dolphin Mole sprite cues, diamond ECB overlay,
stage surfaces, and debug logging. This runtime pass should not move gameplay
authority out of Rust; it is only for playtesting and observing Rust-owned
state.
