# Melee Input And State Reference

Research date: 2026-05-27

This document is a working reference for rebuilding the original project around a
Melee-like input and state engine. The goal is not to copy a whole game. The goal
is to preserve the mechanical lessons that matter for feel: native GameCube
controller semantics, stick/tap timers, 60 Hz deterministic updates, and action
state transitions that are checked in a precise priority order.

## Source Quality

Primary source: the public `doldecomp/melee` decompilation, pinned locally at
commit `c75e4117053da384d314fe69f7c72f9a48dd6c59`. It is a work-in-progress
decompilation of Melee 1.02 `GALE01`, but for the files below it is the best
available public reference for how the engine actually evaluates input and
motion states.

See `docs/research/melee-data-provenance-audit.md` for the current split between
decomp-backed logic, extracted-data paths, and remaining DAT data gaps.

Secondary sources:

- [SSBM Decomp repository](https://github.com/doldecomp/melee)
- [SSBM Decomp generated docs](https://doldecomp.github.io/melee/)
- [GameCube Controller Compendium: Analog sticks](https://compendium.dol-003.info/analog-sticks)
- [Revolution SDK PAD manual mirror](https://pokeacer.xyz/wii/pdf/PAD.pdf)
- [GameCube adapter reverse engineering notes](https://jefflongo.dev/posts/gc-adapter-reverse-engineering/)
- [AltimorTASDK SSBM stick map](https://github.com/AltimorTASDK/ssbm-stickmap)
- [Universal Controller Fix 0.84 official Smashboards page](https://smashboards.com/ucf/)
- [AltimorTASDK UCF source repository](https://github.com/AltimorTASDK/ucf)
- [SmashWiki: Universal Controller Fix](https://www.ssbwiki.com/Universal_Controller_Fix)
- [Melee.guru: Moonwalk](https://melee.guru/characters/tech/moonwalk.html)
- [SmashWiki: Dashdancing](https://www.ssbwiki.com/Dashdance)
- [SmashWiki: Moonwalk](https://www.ssbwiki.com/Moonwalk)
- [libmelee action enum docs](https://libmelee.readthedocs.io/en/latest/index.html)

The decomp source should be treated as a behavioral reference. Before directly
copying source into this project, decide the legal/IP path deliberately. For our
engine work, the safer and more useful path is to implement our own data model
and tests from observed behavior, names, thresholds, and state-machine structure.

## Core Takeaway

Melee input feel comes from a pipeline, not a single stick normalization step.

1. The controller or adapter establishes an origin.
2. The pad library reads raw signed stick/substick axes, analog L/R, and digital
   buttons.
3. The pad library clamps values and scales stick axes by `scale_stick` and
   analog triggers by `scale_analogLR`.
4. Fighter code stores current and previous main-stick and C-stick vectors.
5. Fighter code derives held, pressed, and released button masks.
6. Fighter code updates x/y stick tap timers and trigger timers.
7. The current motion state's input callback, often named `IASA`, checks possible
   transitions in a fixed priority order.

For our engine, the most important implication is that analog max calibration
cannot replace Melee-style input logic. We still need previous-frame values,
tap/smash timers, exact axis deadzones, current state, animation frame, and
transition priority.

## Controller Layer

The GameCube controller has two analog sticks, analog L/R trigger travel, digital
bottom-out L/R buttons, face buttons, Z, Start, and a D-pad. The physical
stickbox matters because it defines the gate and wear pattern, but Melee itself
does not reason about "stickbox" as hardware. The game sees a normalized pad
status after the console/pad layer has interpreted the controller.

Relevant decomp files:

- [`src/sysdolphin/baselib/controller.h`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/sysdolphin/baselib/controller.h#L26-L49)
- [`src/sysdolphin/baselib/controller.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/sysdolphin/baselib/controller.c#L201-L308)

The `HSD_PadStatus` model stores:

- Raw-ish signed axes: `stickX`, `stickY`, `subStickX`, `subStickY`.
- Raw analog values: `analogL`, `analogR`, `analogA`, `analogB`.
- Normalized floats: `nml_stickX`, `nml_stickY`, `nml_subStickX`,
  `nml_subStickY`, `nml_analogL`, `nml_analogR`.
- Button masks: `button`, `last_button`, `trigger`, `release`, `repeat`.

The default pad config in `controller.c` maps to a stick scale of `0x7F` and
analog trigger scale of `0xFF`, so the HSD layer's normalized values are
conceptually signed-stick-over-127 and analog-trigger-over-255 after clamp and
origin handling.

The Revolution SDK PAD manual notes that standard GameCube controllers reset
origin on power-on or insertion, and support X + Y + Start origin reset. The WUP
adapter complicates this because the USB adapter has its own commands and
polling state. Jeff Longo's adapter notes explain that the adapter can expose
origin data, but synchronously retrieving it requires controlling polling/reset
behavior. For this project, "GameCube controller first" means our native adapter
path should preserve raw samples and origin logic before converting to gameplay
values.

Current WUP bridge status: the Rust WUP mapper auto-captures the first connected
raw report as that port's origin, matching the console's power-on/plug-in
behavior. It then subtracts that origin and recenters the native bytes without
endpoint stretching. A worn controller that reaches `232` on the X gate remains
`+104` before UCF/cardinal cleanup; it is not silently stretched to `+127`.
This keeps the input layer GameCube-first and leaves consistency fixes to UCF
and the Melee-style pad processing layer. Raw adapter bytes are still preserved
for readouts and future exact HSD/PAD work.

Native WUP trigger cleanup subtracts the captured L/R trigger origins and applies
the standard PAD trigger rest band before Melee input facts are derived. This
lets holding a trigger during plug-in/launch behave like console origin capture
while preventing small post-origin trigger noise from relatching shield.

## UCF Baseline

Baseline rule: this project targets Melee 1.02 controls with UCF 0.84 behavior
enabled by default, not vanilla-only controller behavior. Smashboards documents
UCF 0.84 as available by default in Slippi and explains the intended design:
make ordinary controllers as consistent as good controllers without making the
techniques stronger than vanilla's best-case hardware. SmashWiki's current UCF
page also identifies 0.84 as the newest fork version as of January 29, 2026.

The implementation boundary matters. UCF is a controller/input consistency layer,
so the controller-side pieces live in Rust, not in the temporary Pygame shell:

- `mole_core::MeleeInputProcessor` keeps a four-entry UCF pad buffer from the
  calibrated native GameCube sample stream.
- UCF 0.84 1.0 cardinals are applied before snapshot output: if one raw stick
  axis is at least `80` units from center and the other axis is within `+-6`,
  the exposed stick becomes a true signed cardinal (`1.0`/`-1.0`) with zero
  cross-axis.
- UCF x tilt intent is exposed when the current raw x sample and the sample two
  input frames earlier differ by more than `75` units. This mirrors the UCF
  dashback tilt-intent check in `include/ucf/pad_buffer.h`.
- UCF shield-drop tilt intent is exposed when the raw y delta over the same
  two-frame window exceeds `44` units. The exact source check also requires
  downward rim position and platform/shield state, so the Rust input layer only
  emits the controller fact; the motion-state interpreter must decide whether
  the current state can consume it.
- The native WUP JSON `melee` object carries `ucf_version`,
  `ucf_x_tilt_intent`, `ucf_dashback_direction`,
  `ucf_shield_drop_tilt_intent`, and `ucf_shield_drop`. The Python bridge only
  forwards these facts for the current prototype.

Important nuance: UCF is input wrapping, not a new movement mechanic. The input
layer owns the raw history and UCF intent facts, then exposes corrected
engine-facing facts such as canonical `dash_direction`. The raw
`ucf_dashback_direction` fact remains available for diagnostics and parity
checks, but gameplay code should not branch on UCF as its own action concept.

## Fighter Input Snapshot

Relevant decomp files:

- [`src/melee/ft/types.h`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/types.h#L1255-L1274)
- [`src/melee/ft/fighter.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/fighter.c#L1759-L2122)

Each fighter stores both current and previous stick values:

- `input.lstick`: current main stick.
- `input.lstick1`: previous main stick.
- `input.cstick`: current C-stick.
- `input.cstick1`: previous C-stick.

It also stores button masks:

- `held_inputs`: currently held buttons.
- `x660`: previous held inputs.
- `x668`: pressed this frame.
- `x66C`: released this frame.

And it tracks timers:

- `x670_timer_lstick_tilt_x`: x-axis tap/smash timer.
- `x671_timer_lstick_tilt_y`: y-axis tap/smash timer.
- `x672_input_timer_counter`: analog trigger timer.

This is the piece we should mirror in the Rust runtime. Our current and previous
derived input snapshots should be part of rollback state. The tap timers should
also be rollback state, not transient UI/controller state.

Current Rust bridge status: `World` stores the previous compact `PlayerInput`
for each player so simulation can derive fresh jump presses from held input
deterministically during rollback/resimulation. `PlayerInput` now preserves the
main stick, C-stick, D-pad, separate L/R analog trigger bytes, separate L/R
digital trigger buttons, separate X/Y jump buttons, attack/special/shield/grab/
start buttons, and its full bit pattern is included in replay/rollback
checksums. `World` also owns per-player Melee-style input timers for x tap,
y tap, and trigger activity, and those timers are included in the checksum.
Fast fall now consumes the core y-tap timer rather than a host-side or
mechanic-specific timer. The richer Melee input snapshot still needs to become
rollback-state data before the Rust core can own dash, tap jump, C-stick edge
semantics, analog shield edge timing, and digital trigger edge timing directly.

Important boundary: compact `PlayerInput` is a calibrated controller-state
packet, not an action-command packet. It stores held physical buttons and axes
for the current frame. Derived facts such as tap jump, fresh A press, fresh X/Y
press, shield edge, and C-stick smash edge belong to `MeleeInputSnapshot` /
`MeleeInputFacts` and the future motion-state interpreter. The stateful WUP
mapper now follows that boundary: it uses the Melee processor for calibrated
current/previous snapshots, but it maps compact input from held physical state
instead of injecting tap jump or one-frame press facts into the packet.
For trigger parity, `PlayerInput::shield()` is only a derived compatibility
view over explicit generic shield intent plus L/R analog or digital trigger
activity. `PlayerInput::explicit_shield()` exposes the generic shield bit by
itself, and the SDL gamepad merge path preserves trigger identity by merging
that explicit bit separately from L/R analog/digital trigger fields.

Current Rust core status: `World::melee_input_snapshot(player, input)` can derive
a `MeleeInputSnapshot` from rollback-owned previous input, current compact input,
and x/y/trigger timers. This means future core motion-state code can consume the
same facts as the runtime readout without depending on WUP-only host state.
The first Rust motion-state slice is now rollback-owned as `Wait`, `WalkSlow`,
`WalkMiddle`, `WalkFast`, `Dash`, `Run`, `RunBrake`, `TurnRun`, `Turn`,
`Squat`, `SquatWait`, `SquatRv`, `SpecialN`, `SpecialS`, `SpecialHi`,
`SpecialLw`, `SpecialAirN`,
`SpecialAirS`, `SpecialAirHi`, `SpecialAirLw`, `AttackAirN`, `AttackAirF`,
`AttackAirB`, `AttackAirHi`, `AttackAirLw`, `Catch`, `Attack1`,
`AttackDash`, `AttackS3`, `AttackHi3`, `AttackLw3`, `AttackS4`,
`AttackHi4`, `AttackLw4`, `GuardOn`, `Guard`, `GuardOff`, `EscapeN`,
`EscapeF`, `EscapeB`, `KneeBend`, `JumpF`, `JumpB`, `Air`, `JumpAerialF`,
`JumpAerialB`, `EscapeAir`, `FallSpecial`, `Landing`, and
`LandingFallSpecial`; state id, state frame, profile walk data, stored turn
target facing, stored jump source, and short-hop flag are all included in the
replay checksum.
The native WUP JSON stream carries that snapshot in its `melee` object, including
cleaned signed sticks, cleaned per-side trigger bytes, timers, and derived
facts. The Python bridge prefers those canonical fields when present and falls
back to raw GameCube bytes only for missing or older stream fields.

## Common Input Thresholds

Relevant decomp file:

- [`src/melee/ft/types.h`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/types.h#L65-L114)

The `ftCommonData` struct holds many shared thresholds. Some fields are still
named by offset in the decomp, so this table maps them by observed use.

| Field | Observed use |
| --- | --- |
| `x0`, `x4` | Main/C-stick x/y deadzone cleanup in fighter input processing. |
| `x8_someStickThreshold`, `xC` | Axis threshold that starts or resets x/y tap timers. |
| `x10` | Analog trigger deadzone. |
| `x14` | Z-button pseudo-shield analog amount: `fighter.c` writes this into `input.x650` after converting held Z into `HSD_PAD_LR | HSD_PAD_A`. The current Mole provisional byte value is 49, matching community controller docs that identify 49 as Melee's Z-lightshield equivalent. |
| `x18` | Analog trigger timer threshold: `fighter.c` advances `x672_input_timer_counter` only when current and previous combined `input.x650` are at or above this value. The current Mole provisional byte value is 140. |
| `x20_radians` | Angle gate used to separate side tilts from up/down tilts. |
| `x24` | Walk entry threshold. |
| `x28`, `x2C`, `x30` | Walk speed bucket thresholds used by walk common code. |
| `x34` | Standing turn threshold. |
| `x38_someLStickXThreshold` | Turn-run threshold. |
| `x3C` | Horizontal smash/dash threshold. |
| `x40` | Horizontal smash/tap window. |
| `x44`, `x48`, `x4C` | Dash-state IASA windows/checks. |
| `x58_someLStickXThreshold` | Run and run-brake threshold. |
| `x60_someFrictionMul` | Turn-run friction multiplier. |
| `x68` | GuardOn catch-dash window copied into `mv.co.guard.x24` after run/dash shield entry. |
| `tap_jump_threshold` | Upward stick threshold for tap jump. |
| `x74` | Tap-jump timer window. |
| `tap_jump_release_threshold` | Short-hop release threshold during jumpsquat. |
| `x80` | Additional jump/knee-bend common-data field referenced by jump code. |
| `x88`, `x8C` | Fast-fall downward stick threshold and tap-window check. |
| `x90` | Crouch threshold. |
| `x94` | Crouch release / SquatRv threshold. |
| `x98` | Forward tilt / throw direction threshold. |
| `attackhi3_stick_threshold_y` | Up tilt threshold. |
| `xCC` | Up smash threshold. |
| `xD0` | Up smash tap window. |
| `xD4` | Down smash C-stick threshold crossing. |
| `xDC`, `xE0` | Aerial neutral threshold and C-stick aerial edge thresholds. |
| `x2A0` | Digital L/R guard window. |
| `x314`, `x318` | Spotdodge stick threshold and tap-window check. |
| `x31C`, `x320`, `x324` | Shield roll stick threshold, tap-window check, and C-stick roll threshold. |
| `escapeair_deadzone`, `x334`, `escapeair_force`, `escapeair_decay`, `x340`, `x344` | Air-dodge vector/deceleration/landing-fallspecial data used by wavedash behavior. `x334` seeds `mv.co.escapeair.timer`; it is an EscapeAir IASA/action timer, not the total animation duration. |
| `x25C` | Fall-special collision/landing condition. |
| `x430` | Run no-interrupt timer seeded when TurnRun resolves into Run. |
| `x440` | Walk common transition field referenced by walk code. |

Exact one-to-one behavior eventually needs the numeric `ftCommonData` table and
character attribute tables, not just these field names. The decomp tells us
which values are compared and where; data extraction tells us the exact numbers.
Timer-window fields are modeled as the source uses them: exclusive upper limits.
For example, a window value of `3` means timers `0`, `1`, and `2` are inside,
while timer `3` is already outside. This matches checks such as
`x670_timer_lstick_tilt_x < x40`, roll's `< x320`, spotdodge's `< x318`, and
tap jump's `< x74`.

Numeric provenance status: the local decomp tree contains the `ftCommonData`
layout and code use sites, but not the extracted `PlCo.dat` data table that
defines the actual field values. Current Rust constants remain centralized
provisional Mole defaults until a real `PlCo.dat` is supplied and extracted;
do not treat a hard-coded threshold as Melee-proven unless it is documented
with the source offset and extracted data source.

Implementation status: `crates/mole_core/src/common_data.rs` now owns the
current provisional threshold values through `MeleeCommonData::provisional_mole`,
including the walk bucket fields `x28`, `x2C`, and `x30` plus the run/run-brake
threshold `x58`, exposes source-offset metadata through
`input_common_data_field_sources`, and has a tested
`MeleeCommonData::from_plco_bytes` extractor for the known
big-endian common-data offsets. The extractor reads real source field types
(`float`, `int`, and `Vec2`) and converts them into the current Rust core units:
normalized stick thresholds use the signed-byte stick scale (`-128..127`),
normalized trigger thresholds use byte trigger scale (`0..255`), frame windows
become integer ticks, `escapeair_force` becomes milli-units, and
`escapeair_decay` becomes percent. `World` now carries `MeleeCommonData`, mixes
it into rollback checksums, and uses it for input tap thresholds, input-fact
thresholds, fast-fall gates, aerial-jump forward/back selection, shield
platform-pass gates, pass/drop-through initial velocity, dash action windows,
run thresholds, and the EscapeAir/FallSpecial common-data slice. This means a
future clean `PlCo.dat` extraction can change those gameplay values through
rollback-owned world data instead of simulator-local constants. The remaining
unit-parity work is moving more of the physics layer from provisional fixed
point values toward Melee's source movement units.
`escapeair_animation_ticks` is intentionally provisional and not extracted from
`PlCo.dat`: source `ftCo_EscapeAir_Anim` leaves `EscapeAir` only when animation
frames are exhausted, so the exact total duration must come from submotion /
animation data rather than the `x334` common-data timer.

## Grounded State Priority

Relevant decomp file:

- [`src/melee/ft/chara/ftCommon/ftCo_Wait.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/chara/ftCommon/ftCo_Wait.c#L43-L65)

`Wait` is the cleanest example of Melee's transition style. The `ftCo_Wait_IASA`
function checks a list of possible transitions with `RETURN_IF` macros. The
first successful transition wins. The order is the behavior.

The broad order from idle is:

1. Specials and high-priority attack helpers.
2. Catch/grab.
3. Smash attacks.
4. Tilts and jab.
5. Shield/guard and appeal/special cases.
6. Jump.
7. Dash.
8. Crouch.
9. Turn.
10. Walk.

For Mole, a state should not ask "what is the best action for these inputs?"
generically. Each state should own an ordered transition function. This is a big
part of why dash, pivot, moonwalk, and microcontrol feel different from a generic
platformer controller.

## Dash, Dash Back, Pivot, Moonwalk

Relevant decomp files:

- [`src/melee/ft/chara/ftCommon/ftCo_Dash.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/chara/ftCommon/ftCo_Dash.c#L30-L83)
- [`src/melee/ft/chara/ftCommon/ftCo_Turn.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/chara/ftCommon/ftCo_Turn.c#L34-L175)
- [`src/melee/ft/chara/ftCommon/ftCo_Run.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/chara/ftCommon/ftCo_Run.c#L24-L132)
- [`src/melee/ft/chara/ftCommon/ftCo_RunBrake.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/chara/ftCommon/ftCo_RunBrake.c#L24-L50)
- [`src/melee/ft/chara/ftCommon/ftCo_TurnRun.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/chara/ftCommon/ftCo_TurnRun.c#L20-L56)

Dash starts when absolute main-stick x reaches the horizontal smash threshold
and the x-axis tap timer is inside the x tap window. If the stick input is
opposite the current facing direction, Melee enters a smash-turn path instead of
a normal forward dash.

The dash state then has its own IASA function. That is where dash dancing and
pivots live mechanically: while still in the initial dash window, the player can
reverse direction through state-local transition checks. After dash becomes run,
opposite stick no longer means "dash dance"; it becomes turn-run or run-brake
style behavior.

Moonwalk is a dash-state consequence, not a separate button command. SmashWiki's
player-facing explanation matches the decomp shape: during initial dash, the
player tilts backward without immediately causing a dash-back state, so horizontal
acceleration/velocity can produce backward slide. It specifically calls out that
the stick is lightly tilted backward for at least two frames before full back.

The input path matters. Melee.guru describes the practical motion as quickly
moving to down-and-away without passing through the center of the analog stick.
Altimor's SSBM stick map gives a useful numeric anchor for the center region:
`DEADZONE = 22` with `CLAMP_RADIUS = 80`, which normalizes to `0.275`. The
important engine rule is not "moonwalk flag on diagonal." It is that a delayed
opposite-side stick can age the x tap timer out before the stick reaches full
back, while a fresh opposite full-back input should still feed the dash-back /
pivot path.

The conceptual rule we are preserving is that moonwalk keeps the dash state and
facing direction, but applies dash acceleration from the current stick side. If
the player dashes left and then routes the stick to the right side of the box
without taking the center/dash-back route, the dash remains a left-facing dash
while rightward velocity is stacked into it. The mirror case should work for a
right-facing dash. If the player reverses through the center route into full
back instead, that is the pivot/dash-back path.

The Python bridge now follows that rule directly. It does not keep a
moonwalk-specific counter or inspect the y axis to decide whether a moonwalk is
allowed. Dash-back/pivot requires a fresh opposite x tap at the high dash/smash
threshold. If the player has already held the opposite side long enough for that
tap timer to expire, the dash state keeps its current facing and the ordinary
dash acceleration responds to the current stick side. The slide is therefore a
physics outcome instead of a hard-coded action.

Follow-through after moonwalk also has to respect Melee's tap timers. In
`ftCo_Dash.c`, dash can hand off to run through `fn_800CA5F0`, but that run
handoff requires the stick to be held with the current facing direction. In
`ftCo_Turn.c`, turn-to-dash only happens when the X tap timer is still fresh.
So a player who moonwalks once and keeps holding the opposite side should not
get a brand-new dash/run merely because the stick remains full back. The bridge
now keeps an explicit X tap timer with two separate gates, matching the shape of
the decomp: `fighter.c` starts/resets `x670_timer_lstick_tilt_x` when L-stick X
crosses the lower `x8_someStickThreshold`, while `ftCo_Dash_CheckInput` only
starts dash when `abs(lstick.x) >= x3C` and that timer is still below `x40`.
This matters for slow stick travel. If the player walks the stick outward over
several frames, the tap timer ages out before the stick reaches the high
dash/smash threshold, so a late full input must remain walk/run intent rather
than becoming a dash. Returning below the lower tap threshold arms the next tap,
and switching sides starts a new tap for dash-back/pivot cases. The current
bridge uses `0.28` for the lower tap-start gate and `0.80` for the high
dash/smash gate until exact `ftCommonData` values are extracted.
The Rust facts layer now treats dash/smash, tap-jump, roll, and spotdodge
windows as exclusive common-data limits rather than inclusive "last accepted
timer" values, so the boundary frame cannot accidentally keep producing fresh
input.

Current Rust core status: `Wait` now consumes the same derived facts for the
first grounded priority slice. Fresh forward x tap enters `MotionState::Dash`,
crouch input enters `MotionState::Squat`, fresh opposite x tap enters the
smash-turn path, and soft opposite stick now enters `MotionState::Turn` through
a separate standing-turn fact modeled after `ftCommonData::x34` instead of
falling through to `Walk`. Same-direction soft stick still enters analog
`MotionState::WalkSlow`, `MotionState::WalkMiddle`, or
`MotionState::WalkFast` from rollback-owned input facts, and once walking has
started a later high stick value does not retroactively become dash if the x tap
timer has aged out. The walk speed bucket cutoffs are centralized in
`MeleeCommonData` with tested `x28`, `x2C`, and `x30` provenance instead of
living as ad hoc movement constants. Fresh forward dash and fresh opposite
smash-turn share the Melee dash check and have priority over crouch; crouch
still has priority over the softer standing-turn check.
The compact Turn IASA slice now accepts grounded side/down/up special, catch,
and attack inputs before shield or jump, matching the decomp's state-local
callback shape. Neutral B is intentionally not accepted during `Turn`: the
decomp's `ftCo_Turn_IASA` calls side-B, down-B, and up-B checkers, but does not
call the neutral-special checker. `Turn` also returns to `Wait` after
`FighterProfile::standing_turn_total_frames`; the fallback Falcon-like value is
11 frames until exact animation-data extraction replaces it. Basic standing turn
now keeps the old facing until the profile-owned
`frames_to_change_direction_on_standing_turn` point (default Falcon-like frame
5), while smash-turn enters with `frames_to_turn = 0` and flips on the next Turn
update rather than at entry.
Rust now stores Melee-shaped turn state in rollback: target facing,
`has_turned`, one-frame `just_turned`, frames-to-turn, dash-out intent, and the
A/B latch. Offensive `Turn_IASA` entries before the flip use target facing,
matching the decomp's temporary-facing check before it restores facing for
shield and jump checks. A/B inputs that are not consumed during Turn can be
latched and replayed on the actual turn frame. Turn now also mirrors the
decomp's `fn_800C9C2C` dash-arm check: a fresh full stick toward
`facing_after` inside the `x40` tap window records dash-out intent, and the
actual Dash occurs only on the one-frame `just_turned` window if the stick is
still held outward at the dash threshold (`x3C`). Entering Dash from Turn
expires the X tap timer, matching the other source-shaped dash entries.

The simplified Rust `Dash` state now preserves facing but applies acceleration
from current stick x. This lets an aged opposite-side input create moonwalk-like
backward velocity without a moonwalk flag, while a fresh opposite x tap during
dash still enters `MotionState::Turn`. Neutral stick during dash now applies
ground traction instead of preserving speed unchanged, matching the source shape
where `Dash_IASA` falls through to friction and `Dash_Phys` applies
accel/target/friction. After Falcon's 15-frame dash window, same-direction stick
exits to `MotionState::Run`, neutral exits to `MotionState::Wait`, and an aged
opposite-side hold exits to the appropriate `WalkSlow`/`WalkMiddle`/`WalkFast`
state instead of becoming a new dash or run. Current Rust `Run` state status:
neutral stick enters
`MotionState::RunBrake`, and full opposite stick enters `MotionState::TurnRun`
using `MeleeCommonData::run_x` / `x58_someLStickXThreshold` instead of a local
simulator threshold, while applying traction instead of opposite run
acceleration on the entry tick.
`TurnRun` now keeps Melee-shaped rollback state for the old-facing
`accel_mul`: entering TurnRun does not flip facing immediately, physics only
accepts opposite acceleration while `accel_mul * accel < 0`, and facing flips
only after ground velocity no longer carries the old facing direction. This
matches the visible `ftCo_TurnRun_Enter`/`Phys`/velocity-crossing shape, while
the exact animation-command pause latch and animation-completion exit timing
still need data extraction. `TurnRun_IASA` only checks the shared jump path in
the visible source, so the Rust state no longer routes shield/roll/walk/run
brake directly out of TurnRun before the source-shaped animation/physics exit.
When TurnRun hands off to Run, Rust now records a provisional `x430`-shaped
no-interrupt timer so immediate neutral/opposite stick cannot become RunBrake
or a fresh TurnRun on the very next frame. The exact `x430` value still needs
extraction from `PlCo.dat`; the current one-frame value is a lower-bound
contract for source ordering, not a final data value.
`RunBrake` now mirrors the source IASA shape more closely: jump and crouch are
available, but forward or soft stick does not immediately cancel back to Run or
walk. When extracted character attributes provide
`max_run_brake_frames`, Rust caps the brake state with that profile-owned value;
the default Falcon-like fallback leaves it unset rather than guessing without
`PlCa.dat` bytes. The simplified Rust dash/run/turn physics still needs the full
Melee acceleration model, exact common-data thresholds, exact TurnRun
animation-script timing, exact Turn animation-duration data extraction, and the
rest of dash/run/turn IASA transitions.

The current rules are:

- Keep the original dash/facing through the moonwalk.
- When dash ends with the stick held opposite, allow turn/settle/walk.
- Treat the horizontal smash/dash threshold as distinct from walk and run
  thresholds. A fast walk input can be above the run threshold but below the
  dash/smash threshold; it must remain walk, not become a fresh dash.
- Do not allow the held stick to create a new turn-dash unless that X tap timer
  is still fresh or the existing dash-pivot path explicitly marked it.
- Once walking starts, settle any leftover dash/moonwalk speed back toward the
  walk target instead of preserving overspeed indefinitely.

Dash and run physics should share the same additive acceleration model used by
the decomp's `getAccelAndTarget` plus `ftCommon_8007C98C` shape:

- Acceleration is based on current stick x and the character's dash/run
  acceleration attributes.
- Target velocity is current stick x times the character's terminal run
  velocity.
- Friction is used when the current velocity would overshoot the target.
- The next state's physics should not run in the same resolver pass that enters
  it; otherwise dash-to-run or dash-to-walk can double-apply movement in one
  frame.

This replaced the legacy multiplicative dash formula and run-speed chunking.
Those older formulas made Captain Falcon values feel worse because the data was
closer to Melee while the engine still interpreted it like a placeholder
platformer controller.

To feel right, our dash state must preserve both:

- State-local timing windows.
- Physics that uses current stick direction during dash, not just a latched
  "dash right" or "dash left" command.
- The current and recent stick path around the neutral square, so moonwalk and
  dash-back are not collapsed into the same full-back input.

## Walk And Crouch

Relevant decomp files:

- [`src/melee/ft/chara/ftCommon/ftCo_Walk.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/chara/ftCommon/ftCo_Walk.c#L33-L105)
- [`src/melee/ft/ftwalkcommon.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/ftwalkcommon.c#L23-L211)
- [`src/melee/ft/ftcommon.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/ftcommon.c#L570-L610)
- [`src/melee/ft/chara/ftCommon/ftCo_Walk.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/chara/ftCommon/ftCo_Walk.c#L89-L104)
- [`src/melee/ft/chara/ftCommon/ftCo_Squat.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/chara/ftCommon/ftCo_Squat.c#L69-L148)

Walk uses character attributes such as `slow_walk_max`, `mid_walk_point`, and
`fast_walk_min`. This means that exact walking feel is character-data driven.
The common code selects walk states and applies shared walk logic, but the
response curve is not only a global input threshold. `WalkSlow`, `WalkMiddle`,
and `WalkFast` are motion-state and animation buckets chosen from current ground
velocity. The walking physics underneath remains analog:

- `target_vel = lstick.x * walk_max_vel`
- `accel = lstick.x * walk_init_vel + sign(lstick.x) * walk_accel`
- shared ground movement then approaches that target with friction/clamping.

`Walk` also has a state-local IASA ladder. In source it checks catch first,
then side special, up special, neutral special, down special, smash attacks,
tilts, jab, shield, crouch/other grounded transitions, jump, dash, turn/release,
and only then walk continuation. This means a held walking stick must not
swallow fresh A/B/Z-style action inputs.

For the Python bridge, this means walking should not quantize input into three
movement speeds. Keep the slow/mid/fast flags as derived animation/debug labels,
but drive velocity from continuous stick X. Nearby values such as `0.20` and
`0.30` should settle to nearby but distinct velocities, and left/right should
mirror.

Current Rust core status: `WalkSlow`, `WalkMiddle`, and `WalkFast` now share
this source-shaped target/approach model instead of assigning velocity directly
from stick X. The walk response values live on deterministic
`FighterProfile` data, with the default Falcon-like profile still using
provisional fixed-point stand-ins until exact character DAT attributes are
extracted. The important engine behavior is in place: first-frame walk velocity
starts below the final analog target, later walk frames continue approaching
that target, and leftover dash/moonwalk speed decays toward the held walk target
instead of being snapped away. Walk buckets are derived from rollback-owned
`MeleeInputSnapshot` facts through `MeleeCommonData` thresholds, and identical
input snapshots plus profile data produce identical walk states and checksums.
This keeps moonwalk follow-through emergent from velocity plus state transition
timing, not from a moonwalk-specific patch.
The walk states now also consume the source-shaped action slice before shield,
jump, or continued walk: catch/grab wins first, then B-specials in walk source
order (side, up, neutral, down), then smashes, tilts, and jab. After shield and
jump, `Walk_IASA` calls `ftCo_Dash_CheckInput`, so the Rust core now lets a
fresh forward dash-strength stick rise enter `Dash` from a walk state, and a
fresh opposite dash tap enter `Turn`; slow stick travel that misses the dash tap
window keeps walking. There is no Walk-only narrower tap window in the decomp:
with the
provisional `x40 = 3` tap window, `Walk` accepts timer frames `0`, `1`, and `2`,
and rejects timer `3` and later. The Rust core now uses that same exclusive
`x_tap < x40` gate and consumes the x tap timer when Walk successfully enters
Dash. The core contract suite pins catch beating special/attack while walking,
walking B-special directions, attack interrupting continued walk, Walk entering
Squat after dash checks, last-valid-frame Walk-to-Dash, and boundary-frame
slow-walk behavior. A soft opposite walk stick now follows the same source
shape as `ft_8008A244`: it exits `Walk` to `Wait` without flipping facing or
continuing left/right walk during that frame. If the opposite stick is still
held on the following frame, the normal `Wait` priority ladder can enter
`Turn`, preserving Melee's turn timing instead of bypassing it through
instant opposite walking.

Crouch starts when main-stick y is below the shared crouch threshold. Like idle
and walk, crouch has its own transition priority for attacks, shield, jump, and
stand/crouch continuations.

Current Rust core status: `Wait` now enters `MotionState::Squat` from down
stick after forward dash and before turn/walk, so diagonal down-walk input
crouches instead of walking while a high forward dash input still keeps dash
priority. `Squat` consumes the same offensive priority slice as `Wait` before
shield and jump, then advances through a profile-owned startup into
`SquatWait`. `SquatWait` holds crouch until the raw stick crosses the
world-owned common-data `x94` release gate, and checks fresh dash before
entering `SquatRv`, matching the decomp's dash-before-release ordering.
`SquatRv` is now explicit and returns to `Wait` after a profile-owned release
duration, while still allowing source-shaped action, shield, jump, and walk
interrupts. Holding down through the y tap window and then pressing A becomes
`AttackLw3` rather than a buffered down smash or continued crouch.

## Jump And Jumpsquat

Relevant decomp files:

- [`src/melee/ft/chara/ftCommon/ftCo_Jump.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/chara/ftCommon/ftCo_Jump.c#L39-L168)
- [`src/melee/ft/chara/ftCommon/ftCo_KneeBend.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/chara/ftCommon/ftCo_KneeBend.c#L20-L78)
- [`src/melee/ft/chara/ftCommon/ftCo_Guard.c`](https://github.com/doldecomp/melee/blob/master/src/melee/ft/chara/ftCommon/ftCo_Guard.c#L398-L467)
- [`src/melee/ft/chara/ftCommon/ftCo_Escape.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/chara/ftCommon/ftCo_Escape.c#L66-L83)
- [`src/melee/ft/chara/ftCommon/ftCo_Escape.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/chara/ftCommon/ftCo_Escape.c#L214-L242)
- [`src/melee/ft/chara/ftCommon/ftCo_Attack100.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/chara/ftCommon/ftCo_Attack100.c#L1253-L1271)
- [`src/melee/ft/chara/ftCommon/ftCo_EscapeAir.c`](https://github.com/doldecomp/melee/blob/master/src/melee/ft/chara/ftCommon/ftCo_EscapeAir.c#L28-L67)

Jump input can come from:

- Tap jump: main-stick y crosses `tap_jump_threshold` while the y tap timer is
  inside the tap-jump window.
- X/Y press: the pressed-input mask includes X or Y.
- Guard-family state checks also allow C-stick up as a jump source through
  `ftCo_800CB024`.

The source of the jump matters. `ftCo_Jump_GetInput` checks tap jump before X/Y,
while `ftCo_800CB024` runs that normal checker first and only then accepts
C-stick up. `ftCo_KneeBend_Check_ShortHop` later tests release differently
depending on the stored source: X/Y short hops when neither X nor Y is held,
L-stick short hops when main-stick y is below `tap_jump_release_threshold`, and
C-stick short hops when C-stick y is below that same threshold.

Melee enters `KneeBend`, the jumpsquat state, before leaving the ground. The
character's `jump_startup_time` determines when the state exits into jump. Short
hop is not a different input at the first frame; it is detected during jumpsquat
when X/Y is released or the stick/C-stick falls below `tap_jump_release_threshold`.
Jumping out of shield still uses this same jumpsquat path: there is no separate
shield rule that forces a short hop or full hop. If jump is held through
jumpsquat, the character full hops; if the jump source is released during
jumpsquat, the character short hops.
Jumpsquat is also not a sealed state. `ftCo_KneeBend_IASA` checks source-level
jump-cancel actions before short-hop polling: up special through the misnamed
`ftCo_Attack100_CheckInput` helper, catch/grab, then up smash through the
no-`xD0` up-smash checker. The up-special route is gated by `fighter.c` setting
the `x686` timer from B+up input, not by the helper name itself. There is no
`EscapeAir` check in `KneeBend`, so airdodge/wavedash still must wait until the
fighter is airborne. That is the source shape behind jump-cancel up special,
jump-cancel grab, and jump-cancel up smash.
In the decomp, ordinary ground states call `ftCo_Jump_CheckInput`, so C-stick up
does not act as a universal jump. `GuardOn`, `Guard`, `GuardOff`, and related
guard-reflect paths call the extended `ftCo_800CB024` helper, which can enter
`KneeBend` from C-stick up after normal jump checks fail. `KneeBend`
initializes short-hop state to false and only sets it when the jump source is
released during jumpsquat. The later jump state then chooses the short-hop or
full-hop vertical velocity from that flag.
Once the fighter is already in jumpsquat, later jump-button changes should not
restart the jumpsquat timer. Re-pressing jump before takeoff can affect whether
the input is currently held, but the `KneeBend` startup itself must continue
from its existing state frame.

Current Python prototype status: jumpsquat now tracks
`jump_released_during_squat` separately from `canJump`. `canJump` gates whether
a new jump may start; short-hop/full-hop selection comes from the jumpsquat
release flag, including jumps out of shield.

Current Rust core status: grounded jump enters `MotionState::KneeBend` and waits
through Falcon's 4-frame jumpsquat before takeoff. The stored jump source is
captured on the transition into `KneeBend`; release of that source during
jumpsquat selects short-hop velocity, holding it selects full-hop velocity, and
re-pressing jump before takeoff does not restart the state timer or spend the
air jump. A fresh normal jump press after the fighter is airborne can spend the
air jump, but held C-stick up alone cannot. `MeleeInputFacts` now separates
`normal_jump_input`/`normal_jump_pressed` from the guard-extended
`jump_input`/`jump_pressed` view so ordinary states match `ftCo_Jump_CheckInput`
while guard states can still use `ftCo_800CB024`. `MotionState::Guard` can
transition into that same `KneeBend` path, so full hop and short hop out of
shield use the same source-release rule as idle jump. The core contract suite
has explicit regressions for shield full-hop and shield short-hop paths, shield
tap-jump and C-stick-jump source preservation, held C-stick not spending an air
jump, held C-stick not becoming a grounded action IASA jump, and takeoff
horizontal velocity. During jumpsquat the Rust core preserves existing grounded
X velocity under traction. On takeoff it applies the Melee-shaped formula:
carried ground speed times a ground-to-air momentum multiplier, plus held
main-stick X times a jump horizontal velocity attribute, clamped by a horizontal
jump max. Ground jump takeoff now enters `MotionState::JumpF` or
`MotionState::JumpB` from the source predicate
`lstick.x * facing_dir > -p_ftCommonData->x78`, rather than collapsing directly
to generic `Air`. Once airborne, normal air drift uses acceleration/friction
toward the held-stick target instead of
snapping horizontal velocity directly to stick X, so jump momentum is preserved
and decays through physics. A fresh aerial jump now enters `JumpAerialF` or
`JumpAerialB` rather than staying in generic `Air`: source
`ftCo_JumpAerial_Enter_Basic` chooses the forward state unless
`lstick.x * facing_dir <= -p_ftCommonData->x78`, then applies air-jump
horizontal and vertical velocity. The Rust core mirrors that state split through
world-owned common-data `air_jump_backward_x`/`x78`, while the jump force and
air-drift values remain profile-owned.
The `KneeBend` IASA slice now mirrors the source priority we can model without
items: up special first, then catch/grab, then up smash, and only then
short-hop release/takeoff. The core contract suite pins digital trigger not
cancelling jumpsquat into `EscapeAir`, up special during jumpsquat, grab
beating up smash, and jump-cancel up smash.

Shield IASA has its own state-local ladder. In source, steady `Guard` checks
shield release first, then item throw, spotdodge, roll, shield-grab, jump, and
platform shield drop. `GuardOn` has the same broad shape but inserts early guard
reflect and a catch-dash window. The current no-item Rust slice preserves the
input-critical order as release, spotdodge, roll, shield-grab, then jump:
release enters `MotionState::GuardOff`; down tap or C-stick down enters
`MotionState::EscapeN`; horizontal tap or C-stick side enters
`MotionState::EscapeF` or `MotionState::EscapeB` based on facing; A or Z while
shield is held enters `MotionState::Catch`; jump enters `MotionState::KneeBend`.
`GuardOn` is now an explicit state. Ordinary standing/walking/squat shield entry
starts in `GuardOn` with no catch-dash window, so A/Z during that startup still
routes to normal `Catch`. Run/dash-style shield entry seeds a small provisional
`mv.co.guard.x24`-style window from common-data `x68`; a fresh A/Z during that
window routes to `CatchDash`, matching the source helper `ftCo_800D8B9C` before
the normal `ftCo_Catch_CheckInput` path. The exact `x68` value still needs
common-data extraction; `GuardOn` duration is profile-owned through
`FighterActionFrames` and remains a fallback action-frame value until exact
animation/action data is extracted.
Shield platform pass now reads its down-stick gate and tap-window from
world-owned common-data `x464`/`x468`, and `Pass` entry uses common-data `x46C`
for the initial drop-through vertical velocity.

`GuardOff` is not a generic `Wait` state. In the decomp, `GuardOff_IASA` first
tries an offensive ladder only when `mv.co.guard.x1C` is set; after that gated
branch, it checks spotdodge and then jump. The current Rust slice does not model
`mv.co.guard.x1C`, so it intentionally models only the always-visible part:
spotdodge before jump, no roll, no dash, and return to `Wait` after a named
profile duration. The 15-frame duration currently matches the local parity
profile through `FighterActionFrames`, but still needs exact animation-data
extraction before being treated as an authoritative source constant. Platform
shield drop, item throw, guard reflect, shield damage, shield setoff, and the
`mv.co.guard.x1C` offensive gate are still future work.

Legacy Python bridge status: held shield remains `blocking`, shield release now
enters an explicit `guardOff` state instead of staying in `blocking` with a
`canBlock`/`dodgeCount` latch. The release frame is consumed by `guardOff`
before the generic jump check, matching the decomp's Guard release priority;
later `guardOff` frames may jump, and the state returns to standing after the
local 15-frame release duration. Directional shield turning is still intentionally
kept while shield is held, but it no longer restarts or extends shield release.

Design exception: directional shield turning is an intentional mechanic, not a
parity bug. In Melee, shield does not allow a grounded facing turn, which means
there is no true turnaround back-air out of shield. This project keeps shield
turning to allow directional shield defense and more out-of-shield attack
routing, but the turn should carry timing risk: a player choosing to face away
for a stronger or better-positioned back-air option must expose the transition
before leaving shield. Future parity work should preserve that risk-reward shape
instead of silently collapsing shield turn back to Melee behavior.

Air dodge is not a jumpsquat action. Wavedash timing comes from waiting until
jumpsquat has ended and then air dodging on the first airborne frame, or later.
`KneeBend` does not check `EscapeAir`; the airborne jump state does. Therefore
a perfect wavedash is modeled as jump, finish jumpsquat, then accept a
directional air-dodge input on the first airborne frame.
Current Rust core status: the reducer ignores digital trigger presses during
`KneeBend`, including the tick that completes jumpsquat. Once the player is in
`MotionState::Air`, a fresh digital L/R bottom-out enters `MotionState::EscapeAir`;
fresh analog trigger travel alone stays shield/lightshield input data and does
not enter air dodge. The core contract suite includes shield-jump versions of
the takeoff-tick, first-airborne-frame, and held-trigger-no-new-edge tests so
holding one trigger for shield and pressing the other trigger for wavedash
timing stays explicit. It also pins the same physical trigger case: analog L/R
held for shield does not hide a later fresh digital bottom-out edge on that same
side once the fighter is airborne. Shield-jump first-airborne priority is also
covered: B-special beats air dodge, and air dodge beats aerial attack.
For the Python bridge this means the resolver must not flip `grounded` to false
and immediately let the same frame's shield edge become an air dodge. The
grounded tick that completes jumpsquat should end with the character leaving the
ground while still in `jumpSquat`; the next tick resolves the jump into `air`,
and only then can a fresh L/R press enter `airDodge`.

Wavedash parity does not end at entering `EscapeAir`. Source `ftCo_EscapeAir`
uses common-data fields for air-dodge deadzone, force, decay, and landing lag
(`escapeair_deadzone`, `escapeair_force`, `escapeair_decay`, and `x344`). Landing
from air dodge goes through the fall-special/landing-fallspecial path rather than
immediately becoming idle. Current Rust status: `EscapeAir` exists and is gated
behind fresh digital L/R while airborne. On entry it now checks
`escapeair_deadzone.x` and `escapeair_deadzone.y` independently, then applies a
fixed `escapeair_force` along the stick angle rather than scaling x/y
independently. Rust now stores source `x334` as
`escapeair_iasa_timer_ticks`, seeds a per-player `escape_air_iasa_timer` on
entry, and lets that timer expire without ending the motion state. EscapeAir
transitions to `FallSpecial` only at animation completion; until animation data
is extracted, Rust uses a separate provisional `escapeair_animation_ticks`
duration so `x334` does not get conflated with the action length again. Entry
sets the source `cmd_skip_decay` flag false, so the Rust core applies
`escapeair_decay` on the entry frame as well as later `EscapeAir` physics ticks,
and skips ordinary falling gravity while in `EscapeAir`. Landing during either
`EscapeAir` or `FallSpecial` enters `LandingFallSpecial` so horizontal slide
survives contact under landing traction. This matches the source callback shape
in `ftCo_EscapeAir_Phys`. Rust now keeps the common-data deadzone as two fields
(`escapeair_deadzone_x` at `0x32C` and
`escapeair_deadzone_y` at `0x330`) instead of collapsing the source `Vec2` into
one scalar. The force, animation duration, decay percentage, and landing
duration are still provisional stand-ins until exact animation data,
`escapeair_force`, `escapeair_decay`, and `x344` behavior are extracted from
`PlCo.dat` / animation data.
`FallSpecial` after air dodge is not input-sealed in source:
`ftCo_FallSpecial_IASA` allows item/parasol hooks, item pickup, and
`ftCo_800CB870` aerial jump. The Rust core now models the non-item air-jump part
by spending the remaining aerial jump from `FallSpecial` and entering
`JumpAerialF`/`JumpAerialB` through the same source-shaped aerial-jump entry
helper used from ordinary airborne state. It also applies normal profile-owned
air drift while `FallSpecial` continues after air dodge, matching the
`xC != 0` branch of `ftCo_FallSpecial_Phys` entered by
`ftCo_80096900(..., 1, 1, false, p_ftCommonData->x340,
p_ftCommonData->x344)`. Exact `x340` mobility handling for other special-fall
entry paths remains pending because those source paths are not represented yet.
The playable Python layer mirrors the same source shape for live controller
testing: `airDodge` uses the two-axis `escapeair_deadzone` check, applies one
fixed-force vector along the stick angle, decays stored self-velocity on the
entry frame and through the source `x334`/IASA-timer movement phase, then stays
in `airDodge` with zeroed self-velocity until the full air-dodge animation count
finishes. Ordinary gravity/drift do not run during either `airDodge` subphase.
Only after animation completion does the Python bridge enter `fallSpecial`,
where gravity resumes.
Landing out of either `airDodge` or `fallSpecial` enters `landingFallSpecial`,
preserves horizontal slide under traction, and uses a separate provisional
landing duration instead of Captain Falcon's four-frame empty landing lag. Rust
now applies ground traction with the same fixed deceleration shape as
`ftCommon_ApplyFrictionGround`, rather than multiplying the slide velocity each
tick. The
decomp path enters this landing state with `allow_interrupt = false`, so a held
L/R trigger from the air dodge cannot route through the grounded guard helper
while `landingFallSpecial` is active, including the frame where the landing state
finishes. If the trigger is still held on the following actionable `standing`
frame, normal grounded shield entry may happen; if the trigger has been released
during the slide, the character remains in standing/walkable control instead of
auto-shielding. `Landing` and `LandingFallSpecial` share the same source
collision callback (`ftCo_Landing_Coll`), so losing floor contact during either
landing state must transition to ordinary `Fall`, not continue the grounded
landing slide with zero vertical velocity. The Python bridge now mirrors that by
exiting `landingLag`/`landingFallSpecial` to `air` when platform contact is lost;
gravity begins on the following airborne tick to match the source order where
landing physics runs before landing collision. The Python constants are still
provisional scale matches; exact values should be replaced by parsed `PlCo.dat`
common attributes once that data path exists.

Current Rust core status: ordinary airborne contact now enters a separate
`Landing` state instead of immediately returning to `Wait`, while `EscapeAir`
and `FallSpecial` contact still enter `LandingFallSpecial`. Held analog shield
does not skip the ordinary landing lag; when the landing state finishes, the
next actionable `Wait` frame may enter `GuardOn` if shield is still held. The
current four-frame ordinary landing duration comes from
`FighterProfile::normal_landing_lag_ticks`; the extractor maps it from
`ftCo_DatAttrs.normal_landing_lag` at `+0xE4` when real character attribute
bytes are available. Landing-state floor-loss now routes to explicit
`MotionState::Fall`, matching `ftCo_Landing_Coll -> ft_80084280 ->
ftCo_Fall_Enter`, instead of preserving `FallSpecial` after the grounded contact
has already been lost.

Current Rust core status: airborne landing and air-dodge landing now route
through a deterministic stage-contact helper instead of a simulator-local
hardcoded `y = 0` floor snap. The helper checks the Battlefield-like main floor
and soft platforms in Rust core units, chooses the highest crossed surface under
the ECB bottom, and preserves the existing Melee state split: ordinary contact
enters `Landing`, while `EscapeAir`/`FallSpecial` contact enters
`LandingFallSpecial`. This is still a vertical-contact slice; ledges, walls,
ceilings, cliff catch, full pass-through platform timing, and source-accurate
collision callbacks must be implemented from the decomp before being treated as
parity.

Horizontal jump velocity uses main-stick x and character jump attributes, then
clamps against character max horizontal jump velocity. This is another reason
input and character data need to meet inside the deterministic simulation, not in
an external input mapper.

## Captain Falcon Test Profile

For local parity testing, `DolphinMole` now uses Captain Falcon as its reference
profile where the Python bridge has a corresponding field. This lets controller
feedback be compared against a familiar Melee character instead of an invented
test character.

Primary stat sources:

- [Captain Falcon (SSBM) stats](https://www.ssbwiki.com/Captain_Falcon_%28SSBM%29#Stats)
- [Dash ranking table](https://www.ssbwiki.com/Dash#Super_Smash_Bros._Melee_rankings)
- [FightCore Captain Falcon frame data](https://www.fightcore.gg/characters/227/captainfalcon/)

Applied movement/stat values:

| Field | Falcon value | Current use |
| --- | ---: | --- |
| Weight | 104 | `weight` |
| Initial dash | 2.0 | `initialDash` |
| Run speed | 2.3 | `runSpeed` |
| Dash frames | 15 | `dashFrames` |
| Dash acceleration | 0.01 stick-scaled, 0.15 base | `FighterProfile::dash_run_accel_stick_per_tick`, `FighterProfile::dash_run_accel_base_per_tick` |
| Walk speed | 0.85 | `walkSpeed` |
| Traction | 0.08 | `traction` |
| Air speed | 1.12 | `FighterProfile::air_drift_max_velocity_per_tick` |
| Air acceleration | 0.02 base, 0.04 add | `FighterProfile::air_drift_base_accel_per_tick`, `FighterProfile::air_drift_stick_accel_per_tick` |
| Air friction | 0.01 | `FighterProfile::air_friction_per_tick` |
| Gravity | 0.13 | `gWeight` |
| Fall / fast fall | 2.9 / 3.5 | scaled into Python bridge velocity units |
| Jumpsquat | 4 frames | `js` |
| Full hop jump force | 3.1 | `FighterProfile::full_hop_jump_force_per_tick` |
| Short hop jump force | 1.9 | `FighterProfile::short_hop_jump_force_per_tick` |
| Full hop height | 38.52 | `fullHopHeight`, converted to `jumpHeight` velocity |
| Short hop height | 14.85 | `shortHopHeight`, converted to `shortHop` velocity |
| Double jump height | 28.56 | `doubleJumpHeight`, converted to `airJumpHeight` velocity |
| Empty landing lag | 4 frames | `FighterProfile::normal_landing_lag_ticks` |
| Air dodge | 49 frames | `airDodgeFrames` |

The current Python bridge stores Falcon move frame data on the character as
`frameData` for implemented or near-term actions: aerials, jabs, tilts, smashes,
dash attack, specials, grabs, spotdodge, air dodge, and rolls. Only the actions
that exist in the bridge can consume those values today. The future Rust core
should move this table into data files and use the full motion-state model
instead of attaching a dictionary to the player object.

The Python bridge currently has public Falcon jump heights rather than extracted
`jump_v_initial_velocity`, `hop_v_initial_velocity`, and air-jump multiplier
values from the character DAT. To keep the current bridge aligned with the
reference profile, it derives initial vertical velocities from those stored
heights under the bridge's gravity step. Once exact DAT attributes are extracted,
those raw velocity attributes should replace the derived bridge values.

Current Rust core status: ground jump takeoff now follows the local decomp shape
from `ftCo_Jump.c`: the profile-owned full-hop or short-hop force is applied as
vertical velocity, horizontal velocity combines carried ground speed with
`jump_h_initial_velocity`, and the result clamps against
`jump_h_max_velocity`. The first airborne tick advances by that force, and the
common fall step reduces stored velocity by profile gravity for the next tick.
Air jump state entry follows `ftCo_JumpAerial.c` by using profile-owned
`air_jump_h_multiplier` and `jump_v_initial_velocity * air_jump_v_multiplier`
attributes. Ordinary airborne drift now follows the `ftCommon_8007D28C` /
`ftCommon_8007D174` shape: main-stick X scales `air_drift_stick_mul`, same-side
input adds `aerial_drift_base`, target velocity scales `air_drift_max`, and
`aerial_friction` is used for neutral input or target overshoot. The
`FighterProfile::from_ftco_dat_attrs_bytes` byte-level path can read those
fields once a local `PlCa.dat`/`ftDataCaptain` attribute block is available.
Dash and run acceleration now follow the shared `getAccelAndTarget` helper from
`inlines.h`: `dash_run_acceleration_a` is scaled by main-stick X,
`dash_run_acceleration_b` is added by input side, and the run target scales from
`dash_run_terminal_velocity` rather than always clamping to full run speed.
RunBrake can now consume extracted `max_run_brake_frames` while leaving the
fallback profile uncapped until real Captain Falcon attributes are available.
Basic standing-turn facing timing now reads the profile-owned
`frames_to_change_direction_on_standing_turn` field.
The full standing-turn lifetime now reads
`FighterProfile::standing_turn_total_frames` instead of a simulator-local Falcon
constant; the value is still a fallback action-frame count until animation data
is extracted.
Attack1 and AttackDash total durations/IASA now read from
`FighterActionFrames`, carried by the fighter profile and covered by rollback
checksums. `GuardOn`, `GuardOff`, `EscapeN`, `EscapeF`, and `EscapeB` total
durations now use the same profile-owned action-frame seam. `Squat` startup and
`SquatRv` release duration are also profile-owned; exact values still need
extracted animation/action data before they should be treated as authoritative.
Remaining grounded action durations and IASA values still need the same
treatment before they can be fed by extracted Falcon action data.
Ordinary grounded `Landing` now also consumes profile-owned
`normal_landing_lag` instead of a simulator constant.

## Fast Fall

Relevant decomp file:

- [`src/melee/ft/ftcommon.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/ftcommon.c#L503-L515)

`ftCommon_CheckFallFast` shows that fast fall is not simply "hold down." The
fighter must not already be fast-falling, vertical velocity must be negative,
main-stick y must be below `-x88`, and the y-stick tap timer must still be
inside `x8C`. When consumed, Melee sets `fall_fast`, resets the y tap timer to
`0xFE`, and applies the character's fast-fall velocity.

For Mole, the input interpreter should only expose the source-backed input fact:
a fresh downward L-stick tap inside the fast-fall window. The Python bridge now
has a y-axis tap timer parallel to the x tap timer and consumes it when fast fall
is applied. Holding down before the character can fast fall no longer becomes a
broad input buffer; the player must create a fresh down tap while falling.

Current Rust bridge status: the deterministic `World` stores per-player
fast-fall tap timers, and each player stores whether they are already
fast-falling. The Rust core now rejects held-down ascent, rejects down held
before falling as a buffered fast fall, and consumes a fresh down tap once while
the player is airborne and descending. The down-stick gate and tap-window come
from world-owned common-data `x88`/`x8C`. When fast fall is active, Rust now
mirrors `ftCommon_FallFast` by setting vertical velocity to
`-profile.fast_fall_speed_per_tick` instead of applying an extra gravity impulse.

Normal fall physics also needs to stay additive. The Rust bridge applies
`gravity * multiplier` each frame and clamps normal fall to the character's fall
speed. Fast-fall velocity is only applied by the fast-fall path, then preserved
as the terminal fast-fall speed.

## Special Inputs

Relevant decomp files:

- [`src/melee/ft/chara/ftCommon/ftCo_Wait.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/chara/ftCommon/ftCo_Wait.c#L43-L48)
- [`src/melee/ft/chara/ftCommon/ftCo_SpecialS.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/chara/ftCommon/ftCo_SpecialS.c#L21-L53)
- [`src/melee/ft/chara/ftCommon/ftCo_Attack100.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/chara/ftCommon/ftCo_Attack100.c#L135-L203)
- [`src/melee/ft/fighter.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/fighter.c#L1729-L1756)
- [`src/melee/ft/chara/ftCommon/ftCo_Fall.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/chara/ftCommon/ftCo_Fall.c#L119-L156)
- [`src/melee/ft/chara/ftCommon/ftCo_JumpAerial.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/chara/ftCommon/ftCo_JumpAerial.c#L89-L120)
- [`src/melee/ft/chara/ftCommon/ftCo_JumpAerial.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/chara/ftCommon/ftCo_JumpAerial.c#L161-L179)
- [`src/melee/ft/chara/ftCommon/ftCo_JumpAerial.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/chara/ftCommon/ftCo_JumpAerial.c#L289-L304)
- [`src/melee/ft/chara/ftCommon/ftCo_SpecialAir.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/chara/ftCommon/ftCo_SpecialAir.c#L17-L61)
- [`src/melee/ft/chara/ftCommon/ftCo_ItemThrow.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/chara/ftCommon/ftCo_ItemThrow.c#L199-L245)
- [`src/melee/ft/chara/ftCommon/ftCo_AirCatch.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/chara/ftCommon/ftCo_AirCatch.c#L59-L92)
- [`src/melee/ft/chara/ftCommon/ftCo_Attack100.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/chara/ftCommon/ftCo_Attack100.c#L409-L438)
- [`src/melee/ft/chara/ftCommon/ftCo_EscapeAir.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/chara/ftCommon/ftCo_EscapeAir.c#L28-L36)
- [`src/melee/ft/chara/ftCommon/ftCo_AttackAir.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/chara/ftCommon/ftCo_AttackAir.c#L61-L83)

Grounded B-special input is directional. The common fighter input counter pass
tracks recent B + side, B + up, B + neutral, and B + down regions separately.
`Wait` then checks side special before up special, neutral special, and down
special. Side special also updates facing when the stick points behind the
fighter far enough. Common airborne IASA paths check B-special first. Within
the B-special helper, air special uses a different direction order: up, down,
side, then neutral. The same `Fall` / `JumpAerial` callback ladder then checks
item-related actions, tether air-catch, fresh digital L/R air dodge, aerial
attacks, and air jump later in the chain. The practical slice for the current
core is that B-special wins over air dodge and air jump, while shield-only
air dodge remains the air-dodge path.

Important source distinction: `ftCo_80095328` is not generic AirCatch. It is
held-item aerial throw/drop routing and returns false when the fighter is not
holding an item. The true `AirCatch` helper, `ftCo_800C3B10`, is Link, Young
Link, and Samus tether behavior only; it rejects other characters, used tether,
active tether callbacks, and held items, then requires held L/R plus a fresh
A/Z-style grab input. For the current Falcon-like parity profile, adding a
generic airborne grab or AirCatch state would be a Mole-specific mechanic, not a
Melee-backed one. Physical Z is still action input in the air because
`fighter.c` maps it into fresh A plus pseudo L/R; for non-tether Falcon-like
characters that reaches aerial attack, not generic `Catch`. The Melee-backed
future work is held-item aerial throw/drop, airborne item pickup probing, and
character-specific tether AirCatch.

Current Rust core status: `MeleeInputFacts` now carries a grounded
`special_direction` and an airborne `air_special_direction` in addition to the
fresh B edge. `Wait`, `Squat`, and the first grounded action-IASA slice resolve
fresh B through the grounded order: side, up, neutral, down. The threshold
edges are intentionally asymmetric like source: side special accepts
`abs(x) >= x218`, up special accepts `y >= x21C`, neutral special requires both
axes strictly inside the side/up thresholds, and down special requires
`y < -x21C`; therefore exactly `y == -x21C` is no grounded B-special route and
falls through to the next state-local checks. `Air` resolves fresh B through the
airborne order: up, down, side, neutral, before air jump or fresh digital L/R
air dodge. This exposes `MotionState::SpecialS`,
`MotionState::SpecialHi`, `MotionState::SpecialN`, `MotionState::SpecialLw`,
`MotionState::SpecialAirS`, `MotionState::SpecialAirHi`,
`MotionState::SpecialAirN`, and `MotionState::SpecialAirLw`; side B updates
facing from the stick direction on entry. Exact `x218`, `x21C`, `x220`, and
`x224` common-data thresholds still need DAT extraction, and exact Captain
Falcon special animation lengths and callbacks still need animation-data
extraction before the placeholder total-frame values can become parity data.

## Dash And Run Action Inputs

Relevant decomp files:

- [`src/melee/ft/chara/ftCommon/ftCo_Dash.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/chara/ftCommon/ftCo_Dash.c#L83-L134)
- [`src/melee/ft/chara/ftCommon/ftCo_Escape.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/chara/ftCommon/ftCo_Escape.c#L85-L92)
- [`src/melee/ft/chara/ftCommon/ftCo_Run.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/chara/ftCommon/ftCo_Run.c#L103-L127)
- [`src/melee/ft/chara/ftCommon/ftCo_AttackDash.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/chara/ftCommon/ftCo_AttackDash.c#L29-L90)
- [`src/melee/ft/chara/ftCommon/ftCo_Attack100.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/chara/ftCommon/ftCo_Attack100.c#L1273-L1290)
- [`src/melee/ft/chara/ftCommon/ftCo_ItemThrow.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/chara/ftCommon/ftCo_ItemThrow.c#L178-L196)
- [`src/melee/ft/fighter.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/fighter.c#L1870-L1893)

`Run_IASA` is not only run/brake/turn logic. Its source order first checks side
special, up special, neutral special, down special, dash catch, and dash attack.
Only after those action routes does it reach guard, jump/appeal-style helpers,
turn-run, the run no-interrupt timer, and run brake. This matters for
shield-plus-A inputs: in running states, A with held L/R is dash grab behavior
before generic guard behavior.

`Dash_IASA` is more windowed than `Run_IASA`. Early dash
(`cur_anim_frame <= p_ftCommonData->x44` in source) checks side special, item
throw, dash catch, side smash, and a short defensive/action helper gated by
`x48`. Plain dash attack is not in this early branch. A later dash window
(`cur_anim_frame <= p_ftCommonData->x4C`) checks side special, dash catch, dash
attack, dash-back, and guard.
The current Rust core should therefore avoid treating Dash as "Run with every
grounded B-special"; early Dash side-B is source-backed, but broad up/neutral/down
B from Dash still needs exact source-window modeling before it should be added.

Catch and dash-catch source helpers are A-plus-held-L/R routes, and physical Z
is normalized into that same shape before common action checks. In `fighter.c`,
held Z sets `HSD_PAD_LR | HSD_PAD_A`, so a fresh Z press reaches
`ftCo_800D8A38` as the same dash-catch intent as A while holding a trigger.
`ftCo_800D8A38` also routes item dash throws before checking character
restrictions for Link, Young Link, and Samus tether state. For the current
Falcon-like profile without held items, the useful deterministic contract is:
fresh Z or fresh A with held L/R while running or in the early Dash route enters
`CatchDash`; fresh A without held L/R from Run or Dash enters `AttackDash`.

Current Rust core status: `MotionState::CatchDash` and `MotionState::AttackDash`
now exist. `Run` resolves grounded B-specials in the source order before shield
or movement continuation, then resolves fresh Z or held-shield fresh A to
`CatchDash`, then fresh A to `AttackDash`. `Dash` now has provisional
`x44`/`x4C` subwindow shape: the first Dash action pass allows source-backed
side-B, dash catch, and early side-smash from A+forward or a side C-stick edge,
then the `x48` defensive helper maps held L/R to `EscapeF`; neutral fresh A is
gated out of `AttackDash` during this early branch. The later Dash window allows
`AttackDash`; a fresh opposite dash-back tap beats the guard helper, while
shield after the defensive window and without a fresh opposite dash tap falls
through to guard.
Exact `PlCo.dat` values for `x44`, `x48`, and `x4C`, item throw routes, guard
helpers, and character-specific catch restrictions remain future parity work.

## Attacks And C-Stick

Relevant decomp files:

- [`src/melee/ft/chara/ftCommon/ftCo_Attack1.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/chara/ftCommon/ftCo_Attack1.c#L46-L159)
- [`src/melee/ft/chara/ftCommon/ftCo_AttackS3.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/chara/ftCommon/ftCo_AttackS3.c#L28-L90)
- [`src/melee/ft/chara/ftCommon/ftCo_AttackHi3.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/chara/ftCommon/ftCo_AttackHi3.c#L22-L69)
- [`src/melee/ft/chara/ftCommon/ftCo_AttackS4.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/chara/ftCommon/ftCo_AttackS4.c#L60-L125)
- [`src/melee/ft/chara/ftCommon/ftCo_AttackHi4.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/chara/ftCommon/ftCo_AttackHi4.c#L29-L71)
- [`src/melee/ft/chara/ftCommon/ftCo_AttackAir.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/chara/ftCommon/ftCo_AttackAir.c#L61-L124)
- [`src/melee/ft/ft_0DF1.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/ft_0DF1.c#L105-L114)
- [`src/melee/ft/ftcommon.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/ftcommon.c#L616-L623)

Jab starts from an A press. Tilts and smashes depend on A press plus stick
direction, angle gates, and tap timers. Side smash and up smash use tap-window
logic; side tilt and up/down tilt use direction/angle thresholds. In the
decomp, side tilt checks A plus facing-side X and requires absolute stick angle
below `x20_radians`; up/down tilt check Y thresholds and require angle above or
below that same gate. Exact one-to-one behavior still needs the numeric
`x20_radians` value from the common data table.

The C-stick is not just another alias for main-stick plus A. It has separate
current and previous vectors, and helpers check C-stick threshold crossings for
smash-style actions. For our controller viewer and runtime, the C-stick must be
displayed and simulated separately from the main stick.

Current Rust core status: the compact deterministic `PlayerInput` stores
C-stick x/y as signed bytes in addition to the main stick. Direct GameCube pad
mapping, calibrated WUP snapshots, SDL right-stick input, and generic
`PhysicalInput` all preserve C-stick values into this packet. The `Wait`,
`Walk`, and `Turn` reducers now consume the grounded attack slice in source
order: side smash, up smash, down smash, side tilt, up tilt, down tilt, then
neutral A. Each smash check can consume either its A+main-stick route or its
C-stick route, so simultaneous A+side-stick plus C-stick-up resolves to side
smash, matching the decomp ladder rather than treating C-stick as a blanket
priority source. These `Wait` and `Squat` action entries happen before shield,
jump, dash/crouch/walk continuations, or release. The exposed grounded action
states now return to `Wait` after the Captain Falcon total-frame counts used by
the local test profile where those counts have been extracted. The first
action-IASA slice now lets Falcon jab, up/down tilt, smashes, and neutral
special resume grounded input priority once their local IASA frame has been
reached; pre-IASA inputs remain locked in the current action. This still does
not claim full attack parity: charge windows, hitboxes, action-specific
callbacks, and follow-ups remain future work.
Diagonal A+stick tilt intent is now reduced to one action instead of falling
through as a two-axis tuple: shallow diagonals become side tilt, while steep
up/down diagonals become up/down tilt. This mirrors the decomp's angle-gate
shape, but the current Rust gate is an axis-dominance stand-in until exact
`x20_radians` data is extracted.

C-stick smash intent should also follow the grounded input priority rather than
reporting every crossed axis as a simultaneous action. `Wait` checks side smash
before up smash before down smash, and the C-stick helpers mirror that shape:
`ftCo_800DF1C8` tests horizontal crossing, `ftCo_800DF2D8` tests upward crossing,
and `ftCo_800DF3A8` tests downward crossing. Therefore a fresh diagonal C-stick
corner should preserve the held C-stick direction as diagonal, but the action
intent should resolve to side smash first. If horizontal was already held and
only vertical crosses later, vertical smash intent can fire on that later frame.

Aerial attacks use a separate selector. `ftCo_AttackAir_CheckItemThrowInput`
starts an aerial from either a fresh A press or a fresh C-stick threshold edge.
If the C-stick edge caused the aerial, direction comes from the C-stick;
otherwise direction comes from the main stick. Both axes below `xDC` / `xE0`
select neutral air, then the shared `x20_radians` angle gate selects up air or
down air, then remaining horizontal input selects forward air or back air based
on facing. Because C-stick-only aerials require crossing the same `xDC` / `xE0`
thresholds, C-stick by itself does not produce neutral air; neutral air is the
A-button plus neutral-stick path.

Current Rust core status: `MeleeInputFacts` now exposes `air_attack_pressed`
and `air_attack_direction`. In `MotionState::Air`, the common airborne priority
slice is B-special first, fresh digital L/R air dodge second, aerial attack
third, and air jump after that. A press can enter neutral/forward/back/up/down
aerials from the main stick. A fresh C-stick edge can enter directional aerials
without A and overrides the main stick for aerial direction. This selector now
uses source-shaped common-data fields: `aerial_neutral_x` maps to `xDC`,
`aerial_neutral_y` maps to `xE0`, and `aerial_vertical_angle_tan_milli` is the
deterministic fixed-point stand-in for `x20_radians`. Exact numeric values are
still provisional until the common data table is extracted.

## Shield And Triggers

Relevant decomp file:

- [`src/common_structs.h`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/common_structs.h#L20-L40)
- [`src/melee/ft/types.h`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/types.h#L1263-L1270)
- [`src/melee/ft/fighter.c` edge masks](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/fighter.c#L1775-L1784)
- [`src/melee/ft/fighter.c` trigger/Z synthesis](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/fighter.c#L1809-L1898)
- [`src/melee/ft/chara/ftCommon/ftCo_Guard.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/chara/ftCommon/ftCo_Guard.c#L460-L469)
- [`src/melee/ft/chara/ftCommon/ftCo_Escape.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/chara/ftCommon/ftCo_Escape.c#L66-L83)
- [`src/melee/ft/chara/ftCommon/ftCo_Escape.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/chara/ftCommon/ftCo_Escape.c#L214-L242)
- [`src/melee/ft/chara/ftCommon/ftCo_EscapeAir.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/chara/ftCommon/ftCo_EscapeAir.c#L28-L36)
- [`PhobGCC analog trigger calibration guide`](https://phobgcc.com/For_Users/Phob_Calibration_Guide_v0.25.html#analog-trigger-value-adjustment---zlzr--dudd)

The GameCube L/R triggers have analog travel and digital bottom-out buttons.
Melee's fighter input processing combines analog trigger activity and digital
L/R presses into shield-related masks and an analog trigger amount. The guard
code checks pressed L/R buttons and held L/R state, and shield size/lightshield
logic uses analog amount. Air dodge is stricter: `EscapeAir` checks fresh
digital L/R button presses, not merely fresh analog trigger travel. In other
words, analog trigger travel can hold/enter shield, but the bottom-out digital
click is the air-dodge trigger. Source `fighter.c` also forces the combined
shield amount (`x650`) to full scale while physical L or R digital is held,
even if the raw analog trigger byte is lower.

Melee separates trigger hold from trigger timing. After the `x10` analog
deadzone cleanup, any nonzero `x650` sets synthetic `HSD_PAD_LR` and can feed
held-shield routing. The analog trigger timer (`x672_input_timer_counter`) is
stricter: it only starts or increments when `x650 >= x18`; otherwise it resets
to expired. That means a light analog trigger value can hold or enter shield
without being treated as an `x18` trigger-timer press.

Physical Z has another source-level wrinkle. In `fighter.c`, held Z is
normalized into `HSD_PAD_A | HSD_PAD_LR`, while preserving the physical Z bit,
then `input.x650` is overwritten with common-data `x14`. This comes after the
digital L/R full-scale assignment, so Z's pseudo-shield amount wins over the
full digital shield amount when both are held. That pseudo L/R feeds catch and
guard-style helpers, but it is not the same as fresh physical L/R bottom-out for
`EscapeAir`, which checks `HSD_PAD_L` or `HSD_PAD_R` directly. Therefore Z can
act like A for aerial attack/catch routing without becoming an air-dodge button.

Edge-order rule: Melee computes `x668` pressed and `x66C` released after the
current `held_inputs` mask has already been source-normalized. Current raw pad
buttons are loaded, physical L/R or nonzero cleaned analog trigger travel adds
synthetic `HSD_PAD_LR`, and held Z adds synthetic `HSD_PAD_A | HSD_PAD_LR`.
Only then does `Fighter_Spaghetti_8006AD10_Inner1` compare the synthesized
current held mask against previous held input state. Code that consumes
`HSD_PAD_LR` can therefore see synthetic holds and edges from Z or analog
trigger travel, while code that consumes physical `HSD_PAD_L | HSD_PAD_R` sees
only true digital bottom-out buttons.

For Mole, this means:

- Store L analog and R analog separately.
- Store L digital and R digital separately.
- Preserve D-pad buttons as independent input bits; do not fold them into
  movement or taunt semantics at the adapter boundary.
- Track per-side L/R press edges, so holding one trigger does not hide a fresh
  bottom-out or analog press from the other trigger.
- Derive a combined `shield_held`/`shield_pressed` view only after preserving
  the original per-trigger values.
- Keep `analog_shield_pressed` and `digital_shield_pressed` separate. Grounded
  shield can use the combined shield view, while air dodge and wavedash timing
  must consume the fresh digital L/R edge.
- Treat Z's combined `analog_shield` amount as common-data `x14`, not as zero
  and not as the full-scale digital L/R amount.
- Preserve separate source-normalized held/pressed/released masks after the
  raw physical mask. This is the safe place for Melee-like helpers that check
  synthesized `HSD_PAD_A` or `HSD_PAD_LR`, while air dodge and wavedash timing
  should continue to use physical L/R digital edges.
- Keep shield entry edge/state-aware. Holding shield should continue the active
  shield state rather than re-entering `blocking` every frame and resetting its
  timers.

Current Rust bridge status: compact `PlayerInput` now stores L/R analog trigger
bytes and L/R digital trigger buttons separately. `shield()` is a derived view
over explicit shield intent, analog trigger pressure, and digital trigger state,
so rollback packets no longer lose which trigger was used or how far it was
pressed. `explicit_shield()` is the generic keyboard/abstract shield bit only;
the SDL gamepad path keeps bumper/trigger input in the L/R trigger fields, and
only keyboard-style shield uses the explicit bit. In the compact rollback path,
zero trigger value is neutral and any nonzero cleaned analog trigger byte is
shield-held trigger activity after adapter origin/deadzone cleanup. The stricter
`trigger_timer_threshold` maps to source `x18`, so low lightshield values remain
shield input while leaving the rollback-owned trigger timer expired. Physical Z
is preserved in the raw button snapshot, but
`MeleeInputFacts` now maps fresh Z to grab plus fresh A plus pseudo shield while
keeping `digital_shield_pressed` and `air_dodge_pressed` false. The Rust
fact layer also exposes source-normalized `source_held`, `source_pressed`, and
`source_released` masks: fresh Z produces source Z, A, and synthetic LR edges;
nonzero cleaned analog trigger travel produces synthetic LR; and physical L/R
bottom-out produces both physical L/R and synthetic LR. These source masks are
derived after preserving raw physical `GameCubeButtonState`, so controller
diagnostics and air-dodge checks can still distinguish analog travel, Z, and
true digital bottom-out.
The runtime JSON readout includes the source mask bits and key source A/Z/LR
booleans beside the raw physical controller state. The Rust motion-state
reducer now consumes
`MeleeInputSnapshot` edge facts for the first shield and air-dodge timing
contracts: analog shield can enter/hold `Guard`, `Guard` can enter spotdodge,
roll, shield-grab, and normal jumpsquat in source priority order, but only
fresh digital L/R can enter `EscapeAir` while airborne. The runtime readout
serializes the cleaned per-side trigger bytes inside `melee.left_trigger` and
`melee.right_trigger`, so host bridges do not need to recalculate trigger
origins or infer side identity from a combined analog shield value. The derived
`analog_shield` fact is the source-shaped combined `x650` shield amount: raw
analog travel when only analog is active, full scale while physical L/R digital
is held, or common-data `x14` while physical Z is held, with the per-side analog
bytes still preserved separately. The provisional `x14` extraction is now
threaded through `MeleeCommonData`/`MeleeInputThresholds` as `z_shield_analog`;
without a local `PlCo.dat`, Mole currently defaults it to byte value 49.

Intentional Mole divergence: shield has a facing direction, and the player can
turn around while shielding. That facing behavior is a project mechanic, not a
Melee parity bug. The Melee-like requirements around it are the timing rules:
shield should still allow full-hop and short-hop jump out of shield through
jumpsquat, and a shield trigger press should only become an air dodge after the
fighter is airborne and the press is a fresh digital L/R bottom-out. Current
Rust core status: while held in `Guard`, soft opposite stick starts a local
5-frame shield turn, flips facing without leaving `Guard`, and clears if the
stick returns to neutral/same-side or if a higher-priority shield action such as
roll, spotdodge, grab, or jump takes over. The shield-turn target and local
counter are rollback state and are covered by checksum.

Python bridge status: the live Pygame launcher consumes Rust `melee` readout
facts when they are present. Analog trigger hold facts drive per-side shield
held state, and per-side digital trigger pressed facts drive the hard
bottom-out edge used for wavedash timing. This is intentionally stricter than a
single combined `blockkey`: a lightshield analog hold may keep shield active,
but the first physical digital L/R click must remain visible as its own edge so
the first airborne frame after shield jumpsquat can enter `airDodge`.

The same bridge now preserves Rust's physical-Z mapping as fact-layer intent:
fresh Z can appear as `attack_pressed`, `grab_pressed`, and pseudo
`shield_held`/`shield_pressed` with `analog_shield == z_shield_analog` for
catch/guard-style routing, while
`air_dodge_pressed` remains false unless a fresh physical L/R bottom-out edge is
present. The live airborne air-dodge gate trusts `melee_air_dodge_pressed` when
that Rust fact exists instead of reconstructing an air dodge from legacy shield
booleans, so Z or stale side-channel trigger data cannot accidentally become a
wavedash input.

## Motion State Model

State inventory and Mole coverage comparison:

- [Melee common fighter state inventory](./melee-common-state-inventory.md)
- [Mole state coverage compared to Melee common states](./mole-state-coverage-comparison.md)

Relevant decomp files:

- [`src/melee/ft/types.h`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/types.h#L861-L890)
- [`src/melee/ft/ftmotionstates.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/ftmotionstates.c#L292-L365)
- [`src/melee/ft/fighter.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/fighter.c#L943-L1390)
- [`src/melee/ft/fighter.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/fighter.c#L1705-L2122)
- [`src/melee/ft/fighter.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/fighter.c#L2160-L2477)

`MotionState` contains:

- Animation id.
- Flags.
- Move id / metadata.
- `anim_cb`.
- `input_cb`.
- `phys_cb`.
- `coll_cb`.
- `cam_cb`.

`Fighter_ChangeMotionState` looks up a motion state by id and installs its
callbacks onto the fighter. The common motion-state table contains `Wait`,
`WalkSlow`, `WalkMiddle`, `WalkFast`, `Turn`, `TurnRun`, `Dash`, `Run`,
`RunDirect`, `RunBrake`, `KneeBend`, `JumpF/B`, `JumpAerialF/B`, `Fall`,
`FallSpecial`, `Squat`, `SquatWait`, `SquatRv`, `Landing`,
`LandingFallSpecial`, `GuardOn`, `Guard`, `GuardOff`, `EscapeAir`, and many
more. Character-specific state tables add character actions on top of the common
states.

State parity rule: if a Melee mechanic depends on a named motion state, Mole
should gain that named state instead of folding the behavior into a generic flag.
That keeps animation end, input/IASA, physics, collision, and rollback state
ownership aligned with the decomp. The current Python bridge is allowed to use
temporary legacy names while we migrate, but each touched mechanic should move
toward explicit Melee-shaped states such as `LandingFallSpecial`, `GuardOff`,
`RunBrake`, and the separate aerial jump/fall variants.

In practice, this means our future Rust core should model a state as data plus
five callbacks or callback-equivalents:

- `anim`: update animation frame, handle animation-end transitions.
- `input`: check IASA / interrupt transitions.
- `phys`: apply velocity, acceleration, friction, gravity.
- `coll`: resolve ground/air/wall/platform transitions.
- `camera`: optional and probably not part of rollback-critical gameplay at
  first.

For rollback, the state id, state frame, animation frame, facing, grounded flag,
velocity, collision-relevant flags, and input timers must be deterministic state.
Current Rust checksum coverage includes the simplified motion state id,
motion-state frame, stored turn target facing, stored jump source, and short-hop
flag so rollback can resimulate turn-facing timing, guard, jumpsquat release
timing, `JumpAerialF/B` selection, first-airborne `EscapeAir` timing, the
`FallSpecial` transition, and the air-dodge landing-fallspecial path
deterministically. The first Rust
action-IASA pass is a table-driven bridge step rather than a full callback
table: states with Falcon IASA data can call back into the grounded
input-priority reducer at/after the IASA frame, but per-action callback behavior
still needs to replace that simplified path.

## State IDs And External Tooling

libmelee exposes Slippi-defined action ids and names, which are useful for
validation, replay comparison, and terminology. Slippi itself is excellent for
observing frame-by-frame inputs and action states, but Slippi data is an
instrumented view of Melee running in Dolphin. The decomp source is the better
reference for implementing the internal transition rules.

Use Slippi/libmelee later to validate "given this input trace, these action ids
and state frames happen," not as the source of the state machine itself.

## Implementation Direction For Mole

The engine should move toward these types:

- `GcRawSample`: adapter status byte, raw buttons, main stick, C-stick, L/R
  analog, per-trigger digital buttons, D-pad.
- `GcOrigin`: main stick, C-stick, L/R origin values and validity.
- `PadNormalized`: HSD-like signed floats and analog floats after origin offset,
  native clamp/deadzone cleanup, and UCF-style consistency fixes.
- `FighterInput`: current/previous main stick, current/previous C-stick, held,
  pressed, released, analog shield amount.
- `InputTimers`: x tap timer, y tap timer, trigger timer, recent button timers.
  Current Rust core status: x/y/trigger timers are now rollback-owned inside
  `World`; recent button timers remain future work.
- `MotionStateId`: common and character state ids.
- `FighterState`: position, velocity, facing, grounded/airborne, motion state,
  animation frame, state frame, character attributes, input timers.

Recommended build order:

1. Preserve raw WUP-028 samples and origin metadata in the input layer.
2. Implement HSD-like pad origin offset, clamp/deadzone, and UCF cleanup as
   deterministic functions with tests.
3. Implement Melee-like `FighterInput` snapshots and edge masks.
4. Implement x/y tap timers exactly enough to support dash, smash, tap jump, and
   SDI-style future behavior.
5. Implement a table-driven motion state model with ordered input callbacks.
6. Rebuild grounded common states first: wait, walk, turn, dash, run, run brake,
   turn run, squat, kneebend, jump.
7. Add golden input traces for dash dance, dash pivot, moonwalk setup, walk,
   crouch, tap jump, short hop, side tilt, side smash, shield press, and
   lightshield.

## Open Questions

- Exact `ftCommonData` values still need extraction from Melee data files or a
  trusted documented table.
- Exact character attributes are required for per-character walk, dash, jump,
  traction, and animation-timing feel.
- We need to decide how close Mole should remain to Melee's quirks. Some quirks
  are the feel; others may be accidental bugs.
- Our "bufferless" design can coexist with Melee-like input if we define
  "bufferless" as no broad future-action buffer. Melee still uses tap timers,
  button edge windows, jumpsquat release detection, and state-local transition
  windows.

## Practical Rule

When a mechanic feels wrong, do not tune only the final velocity or a single
deadzone. Check the whole chain:

raw sample -> origin -> native offset -> clamp/deadzone -> UCF cleanup -> current/previous input ->
pressed/released masks -> tap timers -> state priority -> state physics.

That chain is the feel.
