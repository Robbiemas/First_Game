# Melee Unit Visual Parity Design

## Decision

The Rust engine will use one global Melee-style world unit system for authoritative gameplay. The old Pygame implementation remains useful as an asset, launcher, and visual reference, but Pygame pixel coordinates do not define physics, character size, velocity, friction, gravity, ECB shape, or platform geometry.

The immediate target is a Rust-native playable baseline where Dolphin Mole is rendered as the test character while using Captain Falcon-modeled gameplay values, on a Battlefield-sized default stage with one solid main floor and three soft platforms.

## Goals

- Make core units global and explicit across simulation, collision, character profiles, stage data, logging, replay, and rendering handoff.
- Keep deterministic 60 Hz Rust simulation authoritative.
- Use Captain Falcon as the first reference profile for Dolphin Mole gameplay values and standing-height scale.
- Use a Battlefield-sized default stage as the first reference stage shape.
- Render Dolphin Mole sprites through a camera transform from core units to screen pixels.
- Overlay the Rust-owned ECB polygon on the character so collision scale can be inspected visually.
- Add Rust-side frame logging that explains how WUP input becomes cleaned input facts, motion state, velocity, position, collision contacts, ECB, and render output.

## Non-Goals

- Do not patch Pygame movement logic to create the new authority.
- Do not make the background PNG authoritative for platform coordinates.
- Do not chase pixel-perfect tri-platform art alignment before the core stage scale is correct.
- Do not rewrite the whole engine in one pass.
- Do not introduce an authoritative gameplay server or route 60 Hz gameplay input through Supabase.

## Unit Model

`mole_core` owns gameplay state in Melee-style world units. Positions, velocities, acceleration, traction, gravity, terminal velocity, jump velocities, ECB dimensions, ledges, platform endpoints, collision checks, snapshots, checksums, and replay data must all use the same deterministic unit model.

The runtime renderer owns the conversion from core units to pixels. The renderer may store camera scale, viewport origin, sprite scale, and background alignment, but those values are view data only. They cannot feed back into simulation.

The existing runtime constant-style mapping from world to screen should be replaced by a named render transform with clear direction:

```text
core Melee-style units -> render camera -> screen pixels
```

There should be no hidden second gameplay scale in the SDL runtime.

## Character Profile

Dolphin Mole's first Rust mechanics profile should be Falcon-like:

- movement profile: walk, dash, run, traction, gravity, fall speed, jump squat, jump heights, and related per-frame physics values
- visual scale profile: the standing Dolphin Mole sprite height maps to Captain Falcon's standing height
- animation scale profile: every other Dolphin Mole animation frame uses that same uniform scale, preserving the frame's original proportions
- collision profile: the Rust-owned ECB is a four-point diamond for the active scaled sprite/profile: top, right-middle, bottom, and left-middle

Exact values should come from decomp-backed or extracted Melee data when available. Any interim values must be labeled as provisional Falcon-like values in code and tests so they are easy to replace.

The sprite's raw PNG dimensions are not gameplay units by themselves. The standing sprite establishes the Dolphin Mole-to-Falcon visual scale, and that scale is then applied uniformly to all animation frames. The ECB overlay must come from Rust profile data for the current motion state and active scaled sprite/profile, not from arbitrary SDL rectangles. It should render as a four-sided diamond, not a box, with each vertex touching the midpoint of one side of the current scaled sprite/profile bounds.

## Stage Profile

The default test stage should be Battlefield-sized in core units:

- one solid bottom/main platform that cannot be dropped through
- left, right, and top soft platforms
- ledge and floor extents represented in the same core unit model

The existing `background.png` can be retained as visual dressing and adjusted later. The old Pygame main floor alignment is useful evidence for the asset transform, but the Rust stage geometry should be correct even if the PNG needs manual art adjustment.

## Runtime Visuals

The SDL runtime should draw the Rust-owned state in this order:

1. background image using the render transform or an explicit visual calibration transform
2. stage collision geometry overlay, including solid floor and soft platforms
3. Dolphin Mole sprite scaled from the Falcon-standing-height visual profile
4. four-point diamond ECB polygon overlay from the Rust collision profile and current motion state
5. optional debug text or logging indicator

The visual goal is not final art polish. The goal is to make unit scale, platform boundaries, sprite scale, and ECB shape visible enough to tune correctly.

## Logging

Rust runtime logging should expose one frame-level path from input to output:

- frame number
- raw WUP/GameCube input bytes and buttons
- cleaned/UCF-processed controller state
- semantic input facts such as walk band, dash intent, tap timing, shield pressure, jump press, and C-stick direction
- previous and next motion state
- position and velocity in core units
- collision contacts and platform identity
- four diamond ECB points in core units
- render transform used for the frame

Logs should be machine-readable enough for later replay/debug tooling, with human-readable summaries available during local playtesting.

## Testing

Rust tests remain the authority. This design is ready when tests prove:

- core units are used consistently for state, velocity, ECB, and stage geometry
- Dolphin Mole's test profile uses Falcon-modeled gameplay values
- Dolphin Mole's standing sprite height maps to Captain Falcon height, with uniform scaling applied to other frames
- the default stage geometry is Battlefield-sized in core units
- soft stick from Wait still enters the correct walk band
- dash thresholds and tap timing remain separate from walk thresholds
- sprite scale and the four-point diamond ECB overlay are derived from core profile data and the active scaled sprite/profile rather than arbitrary runtime rectangles
- deterministic snapshots and rollback-owned input frames reproduce the same positions, states, and checksums

Visual verification in the SDL runtime is required after the tests pass, but visual output does not replace deterministic Rust tests.

## First Implementation Checkpoint

The first code checkpoint after this spec is:

1. introduce the explicit core-unit/render-transform boundary
2. add a Falcon-modeled Dolphin Mole profile with documented provisional or extracted gameplay values
3. add a Battlefield-sized default stage profile in core units
4. render the Dolphin Mole sprite and Rust-owned ECB from that profile
5. add frame logs for WUP input through Rust state output
6. lock the changes with focused Rust tests before broadening the engine
