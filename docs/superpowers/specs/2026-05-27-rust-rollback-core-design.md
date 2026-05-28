# Rust Rollback Core Design

## Decision

Move the project toward a Rust-first platform-fighter architecture with a deterministic custom game core, a thin SDL3 runtime shell, and rollback networking shaped around GGPO/GGRS ideas. The existing Pygame prototype stays as a mechanics reference while the new engine is built beside it in a Rust workspace.

The first target is not a content-complete remake. The first target is a fast, testable spine: input in, fixed 60 Hz state transition, snapshot, rollback, replay, render handoff, and transport handoff.

## Non-Negotiables

- Fixed simulation rate: 60 Hz.
- Local player input is applied on the next simulation tick with no artificial gameplay input buffer.
- The core is deterministic and independent of rendering, audio, controllers, files, wall-clock time, sockets, and threads.
- Runtime code may drop frames visually, but it must never mutate authoritative game state outside the core step function.
- Rollback stores compact state snapshots and resimulates from the first corrected frame.
- Replays store initial state, per-frame inputs, and checksums so desyncs can be found without guessing.
- The path must remain zero-cost to build, test, and share.

## Seven-Step Core Architecture

1. Deterministic 60 Hz Core

   `mole_core` owns the rules. It uses integer world units, small value types, packed input, explicit frame numbers, and a single `step_world` function. No floats, wall-clock reads, randomness, IO, SDL, or global mutable state are allowed in this crate.

2. Compact State Snapshots

   Game state is represented as plain Rust data that can be cloned cheaply for now and later moved into byte-packed arenas if profiling proves it necessary. Each snapshot is keyed by `Frame` and can produce a stable checksum.

3. Rollback Ring Buffer

   `mole_rollback` stores recent snapshots, confirmed inputs, predicted inputs, and checksums. When a late input differs from prediction, it restores the snapshot before that frame and resimulates forward to the present.

4. GameCube-First Input Layer

   `mole_runtime` is a thin host around SDL3 and the native WUP-028 path. The GameCube adapter path preserves native controller state first: raw `0..255` main stick bytes, raw `0..255` C-stick bytes, raw `0..255` L/R analog trigger bytes, and GameCube button bits. Compact `PlayerInput`, SDL-style physical input, and Pygame-compatible floats are derived views, not the source of truth.

5. Dumb Renderer

   The renderer receives immutable world snapshots and draws them. It may interpolate visual-only effects later, but it cannot feed visual state back into simulation.

6. Direct No-Cost Transport

   `mole_transport` starts with local loopback and direct UDP LAN/WAN building blocks. Matchmaking, relay, NAT traversal, and account systems are not part of the first core pass because they add cost and complexity before the game feel is proven.

7. Replay and Debug Tools

   `mole_replay` records initial state, input frames, and checksums. Replays become the regression test bed for mechanics, rollback correctness, desync detection, and future balance changes.

## Latency Model

The game should feel immediate locally. The local input sample for frame `N` is consumed by the simulation tick for frame `N` without a separate action buffer. Remote input is predicted when missing; if the actual remote input differs, rollback corrects history and catches up. Any deliberate input delay must be an explicit netplay option rather than the default local feel.

## Slippi Lessons To Borrow

Slippi's useful architectural lesson is not just rollback. It is also the input boundary: preserve the GameCube controller packet shape as long as possible, then let game rules interpret that state frame-by-frame. The useful rollback shape is deterministic simulation, frame-indexed inputs, state restore, replay logs, checksums, and debug tooling that treats desyncs as data.

Dolphin and Slippi code can be used as research references. Directly copying GPL implementation code would require an explicit project licensing decision. The default path is to independently implement the same observable behavior in our Rust core.

## GameCube Input Contract

- `mole_core::GameCubePadStatus` represents the native controller state.
- Neutral main stick and C-stick values are `128, 128`.
- Stick extremes preserve the asymmetric native byte range: `0` maps to `-128`, `255` maps to `+127`.
- L/R triggers retain both analog byte pressure and digital bottom-out button state.
- WUP adapter ports store `GameCubePadStatus` before deriving any compact game input.
- The legacy Pygame bridge can consume raw fields when present, and still supports the older centered fields while the prototype remains runnable.

## Next Input Milestone

Build a Melee-style interpreter above `GameCubePadStatus`. It should consume previous/current pad frames and emit tunable semantic facts such as jump press, dash intent, smash turn, tilt turn, walk zone, fast-fall intent, shield analog pressure, trigger digital click, C-stick direction, and D-pad state. Those thresholds should live in one place so the feel can be measured, adjusted, and eventually compared against Melee behavior frame by frame.

## Initial Workspace Shape

- `crates/mole_core`: deterministic 60 Hz simulation primitives.
- `crates/mole_rollback`: snapshot and resimulation control.
- `crates/mole_replay`: replay log and checksum validation.
- `crates/mole_transport`: no-cost peer transport interfaces and loopback testing.
- `crates/mole_runtime`: executable shell that will host SDL3, rendering, input, and timing.

## First Milestone Definition

The first milestone is complete when `cargo test --workspace` validates:

- 60 Hz tick constants.
- Packed player input semantics.
- Deterministic stepping from identical initial state and inputs.
- Snapshot save/restore by frame.
- Rollback resimulation after corrected input.
- Replay checksum detection.
- Transport loopback input delivery.
- Runtime fixed-step accumulator behavior.
