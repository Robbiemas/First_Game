# Visual Asset And State Graph Migration

The Rust runtime is the authoritative gameplay path. The legacy Pygame project
still contains visual and tuning artifacts that should be preserved and migrated
deliberately.

## Reusable Visual Assets

- `background.png`
- `DolphinMole/standing/*.png`
- `DolphinMole/walking/*.png`
- `DolphinMole/dashing/*.png`
- `DolphinMole/running/*.png`
- `DolphinMole/landingLag/*.png`
- `DolphinMole/airDodge/*.png`
- `DolphinMole/jumpSquat/*.png`
- `DolphinMole/freeFall/*.png`
- `DolphinMole/turning/*.png`
- `DolphinMole/runTurn/*.png`
- `DolphinMole/blocking/*.png`
- `DolphinMole/shield/*.png`

The Rust runtime manifest in `crates/mole_runtime/src/assets.rs` maps Rust
`MotionState` values to these legacy animation groups. That manifest is render
metadata only; mechanics remain in `mole_core`.

## State Graph Viewer

- `tools/state_graph_viewer.py`
- `docs/state_graphs/melee_reference_graph.json`
- `docs/state_graphs/mole_current_graph.json`
- `config/state_graph_layout.json`
- `execs/Open State Graphs.cmd`

Keep this viewer as a side-by-side Melee/Mole state transition reference and
tuning aid. It should help humans inspect state-shape decisions, but it should
not become an authoritative runtime dependency.

## Engine-Agnostic Editor Direction

The state graph viewer and value sheets are development tooling around the
Rust-authoritative core. They should stay plain-data driven so the same
simulation and profile data can later be inspected or edited from a custom Rust
tool, Godot, Unity, or another frontend without changing gameplay authority.
