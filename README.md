# Mole Game

Mole Game is a work-in-progress platform fighter engine. The current project
direction is a native Rust runtime with deterministic 60 Hz simulation,
GameCube-first input, replay validation, and rollback-friendly state.

The original Pygame version is still in the repository as historical reference,
but authoritative gameplay work now belongs in Rust.

## Download The Playtest

For a one-file Windows handoff build, download:

[Download MoleGame-FriendPlaytest.exe](https://raw.githubusercontent.com/Robbiemas/First_Game/278009c3251fb9d1e8b864f251b1e00ff1b4ca10/playtest/MoleGame-FriendPlaytest.exe)

After download, double-click the `.exe`. It extracts the minimal native Rust
SDL3/WUP package under local app data and launches the game. Windows may show a
SmartScreen warning because this is an unsigned early test build.

## Current Status

- Native Rust SDL3 runtime launches and runs the playable test shell.
- WUP-028 GameCube adapter input is supported directly through the native path.
- UCF-style controller preprocessing is enabled by default, with a vanilla
  no-UCF launcher available for controller-path testing.
- Captain Falcon-derived movement, ECB, and source capsule data are being used
  as the first Melee parity target for the Dolphin Mole test character.
- The Melee frame-data pipeline now uses compact source manifests plus generated
  runtime data, instead of treating giant per-frame debug caches as canonical.

## Quick Start

From `D:\Mole Game\First_Game`:

```powershell
cargo test --workspace
cargo run -p mole_runtime -- --frames 120
```

To launch the native game window:

```powershell
.\execs\Run SDL3 Runtime.cmd
```

The first SDL3 launch may download the free SDL3 development package into
`.local/SDL3`. That folder is local-only and ignored by git.

## Recommended Playtest Launchers

Use the scripts in `execs/` for normal Windows playtesting:

- `Run SDL3 Runtime.cmd`: normal native SDL3/WUP playtest, UCF enabled.
- `Run SDL3 Runtime Vanilla No UCF.cmd`: same runtime with UCF disabled.
- `Check WUP Native.cmd`: verifies that the WUP-028 adapter is visible.
- `Monitor WUP Native.cmd`: opens a native GameCube input monitor.
- `Record Native Replay.cmd`: records a deterministic runtime replay.
- `Open State Graphs.cmd`: opens the Mole Game Dev Tool/state graph viewer.
- `Build Friend Playtest Package.cmd`: regenerates the one-file Windows
  handoff build under `playtest/`.

Deprecated Pygame and older generic-controller launchers live under
`execs/depreciated/`.

## GameCube Controllers

The native path is GameCube-first. It preserves raw main stick bytes, C-stick
bytes, split L/R analog triggers, split L/R digital trigger clicks, D-pad, and
buttons before deriving Melee-style input facts.

For a WUP-028 adapter on Windows:

1. Install/select WinUSB for the adapter with Zadig if needed.
2. Close remapper software.
3. Plug in the adapter.
4. Run `.\execs\Check WUP Native.cmd`.
5. Launch `.\execs\Run SDL3 Runtime.cmd`.

UCF preprocessing is default-on for regular playtesting. Use the vanilla
launcher when you need to inspect the pre-UCF native controller path.

## Development Checks

Common checks:

```powershell
cargo fmt --check
cargo test --workspace
python -m pytest tests/test_extract_melee_resources.py tests/test_state_graph_viewer.py -q
```

Short runtime smoke:

```powershell
cargo run -p mole_runtime -- --frames 120
```

SDL smoke:

```powershell
cargo run -p mole_runtime --features "sdl wup" -- --sdl --frames 600
```

## Melee Frame Data And Dev Tools

The project is moving toward a source-shaped Melee data pipeline:

- Compact manifests are canonical.
- Melee XYZ floats are preserved in source-space data.
- Runtime/dev-tool views may flatten into the current 2D presentation layer.
- Large sampled JSON caches are debug/dev-tool artifacts, not canonical storage.

Useful CLI examples:

```powershell
cargo run -p mole_cli -- frame-data extract --all-states --character dolphin_mole --source-character captain --write --json
cargo run -p mole_cli -- frame-data sample --character dolphin_mole --source-character captain --state AttackAirN --frame 7 --json
cargo run -p mole_cli -- frame-data export-runtime --character dolphin_mole --state AttackAirN --output crates/mole_runtime/src/generated/frame_data_boxes.rs --write --json
```

For local Melee resource extraction, see `resources/melee/README.md`. Raw DAT
or ISO files should stay local and must not be committed.

## Repository Layout

- `crates/mole_core`: deterministic simulation, motion states, input facts,
  collision math, checksums, and replay/rollback-owned state.
- `crates/mole_runtime`: SDL/runtime shell, rendering, input source integration,
  debug overlays, and replay/runtime plumbing.
- `crates/mole_input`: native GameCube/WUP input mapping and UCF preprocessing.
- `crates/mole_cli`: JSON-first development helper CLI for agents and tools.
- `crates/mole_replay`, `crates/mole_rollback`, `crates/mole_transport`,
  `crates/mole_signaling`: replay, rollback, transport, and signaling support.
- `tools/`: extraction, graph, Slippi, and generated-data helper scripts.
- `docs/`: architecture notes, research, state graphs, specs, and plans.
- `resources/`: source-derived snapshots and runtime assets.
- `RealMainFile.py` and related Python files: legacy Pygame prototype.

## Legacy Pygame Prototype

The Pygame prototype remains runnable for historical comparison and emergency
reference work:

```powershell
py -3.10 -m venv .venv
.\.venv\Scripts\python -m pip install -r requirements.txt
.\.venv\Scripts\python RealMainFile.py
```

Do not add new authoritative movement mechanics to the Pygame path unless the
task is explicitly about keeping a temporary launcher or reference harness alive.

## Useful Documentation

- `docs/architecture/native-rust-rollback-architecture.md`
- `docs/release_notes/2026-05-31-rust-melee-feel-baseline.md`
- `docs/research/melee-input-state-reference.md`
- `docs/research/mole-state-coverage-comparison.md`
- `docs/superpowers/plans/2026-06-03-melee-3d-collision-sampler-handoff.md`
- `resources/melee/README.md`
- `execs/README.md`

## Guiding Rules

- Rust is authoritative for gameplay.
- Simulation stays deterministic and fixed at 60 Hz.
- Rendering, IO, and host input do not own gameplay state.
- Keep source-derived Melee values as source-shaped floats where the source does.
- Surface parity gaps honestly instead of guessing mechanics.
