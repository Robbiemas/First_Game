# Mole Game

Mole Game is a work-in-progress platform fighter engine. The current project
direction is a native Rust runtime with deterministic 60 Hz simulation,
GameCube-first input, replay validation, and rollback-friendly state.

The original Pygame version is still in the repository as historical reference,
but authoritative gameplay work now belongs in Rust.

## Download The Playtest

For a one-file Windows handoff build, download:

[Download MoleGame-FriendPlaytest.exe](https://github.com/Robbiemas/First_Game/raw/master/playtest/MoleGame-FriendPlaytest.exe)

After download, double-click the `.exe`. It extracts the minimal native Rust
SDL3/WUP package under local app data and launches Friend Connect: the game
window plus a second connection-code window. The visible code is the lobby code,
and each open lobby advertises that code in a small setup-only lobby directory.
Click one of the four open-lobby slots, or press `1`-`4`, to join without typing.
Manual code entry still works as a fallback: type the peer code and press Enter.
The code-owner sees the peer join and can click Start. Start is rebroadcast
briefly as setup signaling so the joining player does not miss it while their
listener finishes connecting. The Friend Connect runtime is play-until-quit by
default, so a player can wait at the code window without a smoke-test frame
timeout.

Friend Connect singles uses lobby role for player ownership: the code-owner is
always P1 and the only player who can start the match; the player who types the
shared code is always P2 and waits for Start. Controllers are inactive at
launch. The first connected local WUP/GameCube controller that produces
non-neutral gameplay input latches as that client's active controller. On the
code-owner machine that active controller drives rollback P1; on the joining
machine it drives rollback P2. Extra local controllers are ignored for this
singles playtest and reserved for a future local-doubles design.

Friend Connect now runs gameplay packet frames from match frame `0`, not from
each machine's window-launch frame. Local controller input follows Slippi's
online-delay model: physical input sampled on match frame `F` is scheduled and
transmitted immediately for game frame `F + delay`, with the default delay set
to `2` frames. The newest `8` future-stamped input frames are retransmitted
each frame until ACKs allow old packets to drop.
For diagnosis or tuning, the delay can be overridden with `--netplay-delay N`.

Supabase is only used for endpoint setup through the public publishable key;
gameplay packets stay direct UDP.
Windows may show a SmartScreen warning because this is an unsigned early test
build.

For solo internet-path testing on one machine, use the packaged
`Run Solo Internet Host.cmd` first, copy the visible Friend Connect code, then
run `Run Headless Internet Peer.cmd` and enter that code. The second launcher
starts a headless P2 that still uses Supabase setup, direct UDP gameplay
packets, the same two-frame delay/repair buffer, and deterministic neutral P2
input. Both roles write compact JSONL diagnostics under `logs\netplay`, capped
at 5 MB per role/session and summarized every 60 gameplay frames instead of
dumping every packet.

The one-click version is
`playtest\MoleGame-LocalInternetPlaytest.exe`. It extracts the same package,
generates a local Friend Connect code, launches the visible P1 host with that
code, and starts a visible P2 peer automatically. Supabase is still used for
setup, while same-machine gameplay packets use explicit loopback UDP ports
`127.0.0.1:41001` and `127.0.0.1:41002` so router NAT hairpin behavior cannot
hide packet-flow failures. Each Friend Connect status panel includes `CPU nHZ`,
which is the approximate uncapped CPU headroom before the deterministic 60 Hz
frame-cap wait.

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

- `Run SDL3 Runtime.cmd`: optimized native SDL3/WUP local gameplay test, UCF enabled; starts the game loop immediately without Friend Connect.
- `Run SDL3 Runtime Vanilla No UCF.cmd`: same optimized local SDL3/WUP runtime with UCF disabled.
- `Check WUP Native.cmd`: verifies that the WUP-028 adapter is visible.
- `Monitor WUP Native.cmd`: opens a native GameCube input monitor.
- `Record Native Replay.cmd`: records a deterministic runtime replay.
- `Open Dev Tool.cmd`: opens the native Rust Mole Game Dev Tool.
- `Build Friend Playtest Package.cmd`: regenerates the one-file Windows
  handoff build under `playtest/`; the packaged default launcher starts Friend
  Connect, while local-practice and solo internet headless-peer launchers remain
  in the extracted folder.

The repeatable agent-facing package command is:

```powershell
cargo run -p mole_cli -- package friend-playtest --json
```

To explicitly compose the secondary solo internet launcher on demand:

```powershell
cargo run -p mole_cli -- package local-internet-playtest --json
```

That command builds the release SDL3/WUP runtime, regenerates the one-file
handoff exe plus the local internet playtest exe, extracts it, checks the
expected packaged files, and starts the packaged runtime without the repo asset
root. Use this instead of manually remembering the packaging script.

To inspect the current Friend Connect contract and package artifact status
without rebuilding:

```powershell
cargo run -p mole_cli -- friend-connect status --json
```

That status report also records the current netplay contract: default
`2`-frame delay, `8` recent input frames retransmitted, direct UDP gameplay, and
rollback correction for late remote inputs while snapshots are still retained.
The delay is Slippi-style future-frame input scheduling, not a delay-based wait
for old inputs.
It also records the solo internet test contract for the visible-host plus
headless-peer harness.

Deprecated Pygame and older generic-controller launchers live under
`execs/depreciated/`.

## GameCube Controllers

The native path is GameCube-first. It preserves raw main stick bytes, C-stick
bytes, split L/R analog triggers, split L/R digital trigger clicks, D-pad, and
buttons before deriving Melee-style input facts.

For a WUP-028 adapter on Windows:

1. Install/select WinUSB for the adapter with Zadig if needed.
2. Install the GameCube adapter polling-rate overclock driver on that Windows
   machine. Without it, local WUP input can feel delayed even when network play
   feels snappy from another computer.
3. Close remapper software.
4. Plug in the adapter.
5. Run `.\execs\Check WUP Native.cmd`.
6. Launch `.\execs\Run SDL3 Runtime.cmd`.

UCF preprocessing is default-on for regular playtesting. Use the vanilla
launcher when you need to inspect the pre-UCF native controller path.

For Friend Connect singles, plugged-in controllers do not claim gameplay just by
being present. Move the stick or press a button on the intended controller after
the runtime is open; that first non-neutral local controller becomes the active
controller for that machine until it disconnects.

## Development Checks

Common checks:

```powershell
cargo fmt --check
cargo test --workspace
python -m pytest tests/test_extract_melee_resources.py tests/test_state_graph_viewer.py -q
```

Cargo accepts only one test-name filter per `cargo test` invocation. When
checking multiple exact tests, run separate commands, or use one shared
substring/module filter that intentionally matches all of them.

Short runtime smoke:

```powershell
cargo run -p mole_runtime -- --frames 120
```

SDL smoke:

```powershell
cargo run --release -p mole_runtime --features "sdl wup" -- --sdl --frames 600
```

Uncapped SDL perf smoke:

```powershell
cargo run --release -p mole_runtime --features "sdl wup" -- --sdl --frames 1800 --timing --no-frame-cap
```

The SDL/WUP runtime requests high-resolution host sleep timing on Windows and
paces frames against the remaining 60 Hz budget after update/render work. Do not
restore a full `TICK_NANOS` sleep at the end of the loop; that makes frame time
equal to work plus 16.67 ms and causes visible lag. Current SDL pacing
spin-waits to a rolling fixed 60 Hz frame deadline because coarse Windows host
sleeps were observed to overshoot the cap by roughly 1-2 ms on test hardware.
Keep the rolling deadline: starting a fresh full tick after an overslept frame
locks the runtime to the overslept cadence instead of recovering the 60 Hz
average.
Normal play remains capped for deterministic 60 Hz simulation, but `--timing`
now reports `avg_work_ms` and `uncapped_work_fps` separately from sleep. The
uncapped workload baseline is 240 FPS, equal to four players at 60 Hz, so
`avg_work_ms` should stay below 4.167 ms before the cap is applied.
Future 120/240 Hz backend render/input-sampling experiments should feed rollback
detection and prediction only; they must not change the GameCube/WUP input
mapping or the decomp-shaped 60 Hz input snapshot consumed by core gameplay.
Those higher-rate passes may receive packets, choose the next frame's committed
input, and prepare/resimulate whole 60 Hz rollback frames before presentation,
but authoritative fighter state, collision, damage, and animation output remain
the same 60 Hz sequence.
Rollback snapshots are core-owned and restore in place through
`WorldRollbackSnapshot`; do not snapshot cloned `World` values. Static
configuration such as stage profiles, Melee common data, fighter profiles, and
baked source tables belongs to the session/runtime, while per-frame rollback
stores only mutable authoritative gameplay state.

## Melee Frame Data And Dev Tools

The project is moving toward a source-shaped Melee data pipeline:

- Start with `docs/architecture/rust-devtool-lossless-middleware.md` before changing
  decomp extraction, Mole CLI, the Rust dev tool GUI, or engine import paths.
- Use `docs/architecture/rust-devtool-ui-primitives.md` before adding or copying
  dev-tool layout behavior. Shared panel, table, height, status, and theme fixes
  belong in reusable Rust primitives, not one tab.
- Compact manifests are canonical.
- Melee XYZ floats are preserved in source-space data.
- Runtime/dev-tool views may flatten into the current 2D presentation layer.
- Large sampled JSON caches are debug/dev-tool artifacts, not canonical storage.
- The Rust CLI/dev tool is the preferred path for any pipeline stage that feeds
  the Rust engine. Existing Python scripts are legacy/reference tooling and
  should be ported when touching them is cheaper than extending them.
- The Mole CLI and Rust dev tool GUI are dual surfaces over the same Rust-owned
  artifacts. Any feature exposed on one side should be added to the other side,
  and engine-fed artifacts must preserve source provenance, raw values,
  converted values, gaps, and explicit overrides.
- Canonical Melee action-state ids remain authoritative even when there is no
  Rust `MotionState` alias. Common Damage and DamageFly states 75-91 plus
  Passive/PassiveStand states 199-201 bind to their source FigaTree actions as
  `motion_state: None`; do not add compatibility enum variants just to make
  those poses render.
- The Entry family is not a source-data gap: `Entry`, `EntryStart`, and
  `EntryEnd` keep distinct Melee action-state ids `322`, `323`, and `324`, but
  all resolve to Captain Falcon source action table id `238` / source key
  `Entry` for runtime pose, ECB, and hurt capsule coverage.
- Runtime damage timing follows the decomp split: `hitlag_frames` models
  `Fighter.dmg.x195c_hitlag_frames` and freezes the fighter tick while it
  counts down, while `damage_hitstun_frames` models
  `mv.co.damage.x0 = (int)(kb_applied * p_ftCommonData->x154)` for source-only
  Damage/DamageFly action lockout. Runtime also carries baked
  `source_action_total_frames` from the compact export into core state so
  Damage/DamageFly exits follow `!ftAnim_IsFramesRemaining && !x221C_b6`.
  Damage/DamageFly air physics follows the same split: while locked it uses the
  `ft_80084EEC` gravity plus aerial-friction path; after the lockout clears and
  before animation exit it uses the ordinary `ft_80084DB0` fall/drift path.
  Ordinary Damage ids 75-86 also run the decomp floor-contact slice of
  `ftCo_Damage_Coll`: extracted CommonAttributes `x1E0 = 5.0` and
  `x1E4 = 0.5` gate the branch, the middle band
  `0.5 <= |x8c_kb_vel| < 5.0` enters basic `Landing`, and high knockback enters
  canonical source-only `DownBoundU`/`DownBoundD` action ids `183`/`191`. The
  U/D choice is driven by baked `FtPart_HipN` matrix components from
  `source_frame_capsules.bin`, matching `ftCo_80097570` for normal-fighter
  flags. DownBound animation end now enters canonical source-only
  `DownWaitU`/`DownWaitD` action ids `184`/`192` and seeds
  `mv.co.downwait.x0` from extracted CommonAttributes `x424`; timer expiry then
  enters canonical source-only `DownStandU`/`DownStandD` action ids `186`/`194`
  through baked `DownStand` source data. DownWait's visible IASA slice now also
  routes fresh source-normalized `HSD_PAD_A | HSD_PAD_B` to baked
  `DownAttackU`/`DownAttackD` action ids `187`/`195`, and fresh
  source-normalized `HSD_PAD_LR` to `DownStandU`/`DownStandD`. DamageFly and
  DamageFlyRoll floor contact now mirrors the represented decomp order:
  `ftCo_80090184` / `ftCo_DamageFlyRoll_Coll` attempts PassiveStand then
  Passive through `ftCo_800986B0` using rollback-owned `x680`/`x684` digital
  L/R timers and extracted CommonAttributes `x1C`, `x250`, and `x254`; if those
  checks fail, it falls through to `ftCo_80097D40`, entering baked
  `DownBoundU`/`DownBoundD` through the same hip-pose gate. The Hammer-item
  veto in `ftCo_800C5240`, wall/ceiling passive callbacks, exact
  PassiveStand model-velocity physics, DownWait side getup/roll routing,
  vertical-stick stand-up thresholds, and downed damage/passive callbacks remain
  explicit parity gaps.
  Those timers and frame counts are rollback-owned core state, not render-only
  metadata.
- The CLI/dev-tool export evaluates compact source FigaTree/JObj data and
  writes `source_frame_capsules.bin`, a baked action/frame capsule and
  DownBound hip-pose sidecar.
  Player runtime only decodes that sidecar during
  `preload_runtime_source_frame_data`; gameplay, rendering, collision, and
  startup must not sample or evaluate source FigaTree/JObj data.

Useful CLI examples:

```powershell
cargo run -p mole_cli -- frame-data extract --all-states --character dolphin_mole --source-character captain --write --json
cargo run -p mole_cli -- frame-data sample --character dolphin_mole --source-character captain --state AttackAirN --frame 7 --json
cargo run -p mole_cli -- frame-data export-runtime --all-states --character dolphin_mole --write --json
```

`export-runtime --all-states` writes the compact runtime source asset under
`crates/mole_runtime/src/generated/source_frame_data.rs` with generated
`source_frame_data/` JSON/bin sidecars. The CLI/dev-tool middleware may point at
the local decomp/raw extracts while exporting, but the game only consumes these
compile-time included Rust assets at runtime. It does not read raw Melee DAT
files during play. `preload_runtime_source_frame_data` decodes the generated
`source_frame_capsules.bin` sidecar into the in-memory cache; it must not parse
or evaluate the embedded compact source export.

Neutral `Wait` advances through the decomp animation-frame lane (`cur_anim_frame`
equivalent) every fighter tick; render sprites and source capsule sampling must
read that animation pose frame, not a stale compatibility state-age frame.

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
- `tools/`: legacy extraction, graph, Slippi, and generated-data helper scripts
  pending Rust migration where they feed engine data.
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
- Keep source-derived Melee values as source-shaped floats where the source
  does; milli-integer/fixed-point values are not authoritative substitutes.
- Surface parity gaps honestly instead of guessing mechanics.
