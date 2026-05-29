# First Game

Native Rust rollback platform-fighter work-in-progress, descended from a small
Pygame prototype.

This repo has two tracks:

- `crates/`: the Rust rollback architecture and normal development path.
- `RealMainFile.py`: the original Pygame prototype, kept runnable as a
  historical reference and temporary visual/QA harness.

Authoritative gameplay work should happen in Rust. Pygame should not receive
new movement mechanics except when a small launcher or harness fix is needed.

## Setup

Use Python 3.10 on Windows:

```powershell
py -3.10 -m venv .venv
.\.venv\Scripts\python -m pip install -r requirements.txt
```

## Run The Legacy Pygame Prototype

All double-click launchers and checkers live in `execs/`.

Double-click `execs\Launch Mole Game.cmd`, or run:

```powershell
.\.venv\Scripts\python RealMainFile.py
```

The legacy prototype opens in a window by default. Set `MOLE_FULLSCREEN=1` for fullscreen.
With a WUP-028 adapter on WinUSB, the launcher also starts the native Rust WUP input bridge so the old Pygame prototype can read the GameCube controller without a remapper. Set `MOLE_DISABLE_NATIVE_WUP=1` to force the legacy Pygame/keyboard path only.
The old prototype menu is skipped by default during mechanics QA so native input starts immediately; set `MOLE_SHOW_MENU=1` if you want to open that menu again.

For controller/mechanics diagnosis, double-click `execs\Launch Mole Game Debug.cmd`. The debug launch starts with the input/state overlay visible and writes frame-by-frame JSONL logs under `logs/`. While the game is running, press `F3` to toggle the overlay and `F4` to toggle logging.

For live QA while iterating, double-click `execs\Live Test Session.cmd`. It launches the game with debug overlay/logging forced on, watches Python/config/Rust source changes only while the game is open, and exits the watcher when the game closes or crashes. New logs stay under `logs/`. The live session and the game entry point both hold single-instance locks, so a second launch exits before it can create another watcher or game window. Source changes are reported in the console but do not restart the game by default; pass `--auto-restart` only when you explicitly want the older replace-on-edit behavior.

## Rust Rollback Path

The forward engine direction is Rust with a deterministic 60 Hz core, rollback snapshots, replay validation, no-cost transport primitives, and an SDL3 runtime shell.

Install Rust for free through Rustup, then run:

```powershell
cargo test --workspace
cargo run -p mole_runtime -- --frames 120
```

Useful Rust checks while developing:

```powershell
cargo fmt --check
cargo test -p mole_core
cargo test -p mole_transport --features webrtc
cargo test --workspace
```

## Native Rust Playtest Checklist

From `D:\Mole Game\First_Game`:

```powershell
cargo test --workspace
cargo run -p mole_runtime -- --frames 120
powershell -NoProfile -ExecutionPolicy Bypass -File tools\setup_sdl3.ps1
cargo run -p mole_runtime --features "sdl wup" -- --sdl --frames 600
cargo run -p mole_runtime -- --record-replay --frames 600
```

For GameCube/WUP checks:

```powershell
.\execs\Check WUP Native.cmd
.\execs\Monitor WUP Native.cmd
.\execs\Run WUP Native Runtime.cmd
```

For visual state reference:

```powershell
.\execs\Open State Graphs.cmd
```

The SDL runtime now draws the checked-in `background.png` and DolphinMole PNG
frames from Rust-owned sprite cues, with deterministic rectangle fallback data
kept for tests and diagnostics.

You can also double-click `execs\Run Rust Runtime.cmd` for the current Rust smoke run. It advances a deterministic 120-frame simulation and prints the final frame plus checksum.

For the native SDL3/WUP game window, double-click `execs\Run SDL3 Runtime.cmd`.
The first launch downloads the free SDL3 development package into `.local/SDL3`,
which is ignored by git.

For the native WUP-028 GameCube adapter path, double-click `execs\Check WUP Native.cmd` to verify ports, then `execs\Run WUP Native Runtime.cmd` to run the 60 Hz smoke loop using WinUSB/libusb directly. This path does not require a controller remapper.

For a Delfinovin-style native input display, double-click `execs\Monitor WUP Native.cmd`. It opens a lightweight SDL3 window that polls the WUP-028 adapter directly through WinUSB/libusb and shows connected adapter ports, main stick, C-stick, D-pad, separate L/R analog trigger values, separate L/R digital trigger clicks, and button state for the first two connected controllers.

The WUP path is now GameCube-first internally. Rust stores the adapter report as native `0..255` GameCube stick bytes, native `0..255` trigger bytes, and GameCube button bits before deriving any legacy `PlayerInput` or Pygame-compatible float axes. The compact Rust `PlayerInput` now preserves main stick, C-stick, D-pad, separate L/R analog triggers, separate L/R digital trigger clicks, separate X/Y jump buttons, and gameplay buttons for deterministic replay/rollback checksums. The JSON stream keeps the older centered fields for fallback tooling, emits `raw_main_x`, `raw_main_y`, `raw_c_x`, and `raw_c_y`, and includes a `melee` object with canonical cleaned sticks, cleaned per-side triggers, timers, and input facts. The current Pygame bridge prefers that `melee` object when present, then falls back field-by-field to the raw stream for older runtimes or partial data.

The Rust native input layer now treats UCF 0.84 as the default Melee control baseline. UCF cardinals, raw two-frame x tilt-intent, and raw shield-drop tilt-intent are derived in `mole_core::MeleeInputProcessor` from the calibrated WUP samples before the game shell sees them. UCF dashback is folded into the canonical `dash_direction` fact at the input boundary, while `ucf_dashback_direction` remains available as a diagnostic/raw-fix fact. The Pygame prototype consumes the canonical facts and does not implement UCF-specific movement branches.

`PlayerInput` is intentionally a per-frame controller-state packet, not a pre-resolved action command. The WUP mapper keeps held A/B/X/Y/Z/Start, sticks, D-pad, and trigger state in the packet; Melee-style derived facts such as tap jump, fresh button presses, and shield edges stay in the Melee snapshot/readout layer for the engine to consume deterministically. `shield()` is a derived view, while `explicit_shield()` is only the generic keyboard/abstract shield bit; SDL gamepad trigger and bumper input now stays as L/R analog or L/R digital trigger state instead of being promoted into that generic shield bit.

Native WUP input is plug-and-play by default. The first connected raw GameCube report becomes that controller port's origin, matching the console-level power-on/plug-in behavior: if a stick or trigger is held during launch or insertion, that value is treated as the origin just like on console. Hold `X + Y + Start` for about three seconds to recenter the current stick and trigger origin without restarting. The native path now subtracts origin only; it does not apply user endpoint calibration or stretch real GameCube gate values into a fake full square. UCF/cardinal cleanup and Melee-style input facts happen in Rust before the Pygame shell sees the controller.

The legacy Pygame input bridge now applies a `0.20` trigger dead zone before exposing L/R analog values to movement or shield logic: native values from `0.00` through `0.20` become `0.00`, then the remaining trigger travel is rescaled back across `0.00..1.00`. Low trigger noise is treated as no trigger input instead of being blocked later by special-case action rules.

Architecture docs:

- `docs/superpowers/specs/2026-05-27-rust-rollback-core-design.md`
- `docs/superpowers/plans/2026-05-28-native-rust-rollback-migration.md`
- `docs/architecture/native-rust-rollback-architecture.md`

The Rust core rules are intentionally strict: 60 Hz fixed tick, no gameplay input buffer, no rendering or IO in simulation, compact per-frame input, snapshot-based rollback, and replay checksums for desync detection.

The deterministic `World` owns previous per-frame input plus Melee-style x tap, y tap, and trigger timers. Those timers are part of rollback/replay state and checksum coverage, so host input layers should preserve controller state and leave input edge/timer interpretation to the core.

The core can now derive a Melee-style input snapshot from rollback-owned state with `World::melee_input_snapshot(player, input)`. Runtime readouts and future motion-state code can use the same snapshot/facts shape instead of interpreting host input differently. In that compact rollback path, any nonzero cleaned analog trigger value counts as analog shield hold, the stricter source-shaped `x18` threshold drives the trigger timer, and the physical bottom-out L/R clicks remain separate digital presses for air-dodge and wavedash timing.

UCF intent bits are also carried inside rollback-owned `PlayerInput`, so future Rust motion states can apply UCF dashback/shield-drop behavior deterministically during rollback instead of depending on Pygame or host-only controller state.

Input timer windows now use Melee-style exclusive common-data limits: a window of `3` accepts timers `0`, `1`, and `2`, and rejects timer `3`. Dash/smash-turn, tap-jump, roll, and spotdodge facts all use that same strict boundary shape.

Input thresholds now route through `MeleeCommonData::provisional_mole()` in `mole_core`, with source-offset metadata exposed by `input_common_data_field_sources()`. The current numeric values are still Mole defaults, but fields like dash `x3C`, dash window `x40`, run/run-brake `x58`, tap jump `0x70`, spotdodge `0x314`, and roll `0x31C` are now named in one place so we can swap in extracted `PlCo.dat` values cleanly instead of changing mechanics code by hand.

Aerial attack direction has the same source-shaped split: `xDC` and `xE0` now drive Aerial neutral-zone and fresh C-stick aerial edge detection, while `x20_radians` is represented as a deterministic fixed-point angle gate until the exact DAT value is extracted.

Jump input is now state-local in the same shape as the decomp: ordinary jump checks use tap-jump or X/Y only, while guard-family checks can additionally accept C-stick up through the guard-extended jump view. This keeps held C-stick up from spending an air jump or canceling grounded action IASA as a normal jump, while preserving C-stick jump out of shield.

The Rust simulation now has the first rollback-owned motion-state slice: `Wait`, `Walk`, `Dash`, `Run`, `RunBrake`, `TurnRun`, `Turn`, `Squat`, `SpecialN`, `SpecialS`, `SpecialHi`, `SpecialLw`, `SpecialAirN`, `SpecialAirS`, `SpecialAirHi`, `SpecialAirLw`, `AttackAirN`, `AttackAirF`, `AttackAirB`, `AttackAirHi`, `AttackAirLw`, `Catch`, `Attack1`, `AttackS3`, `AttackHi3`, `AttackLw3`, `AttackS4`, `AttackHi4`, `AttackLw4`, `Guard`, `GuardOff`, `EscapeN`, `EscapeF`, `EscapeB`, `KneeBend`, `Air`, `EscapeAir`, `FallSpecial`, `Landing`, and `LandingFallSpecial`. `Wait` uses Melee-style input facts rather than generic stick speed: fresh B resolves through grounded B-special priority as side, up, neutral, then down; Z enters `Catch`; fresh C-stick smash and A+stick smash/tilt/jab intents enter their matching attack states; fresh forward x tap enters `Dash`; down stick enters `Squat`; fresh opposite x tap enters the smash-turn path; soft opposite stick enters standing `Turn`; same-direction soft stick enters analog `Walk`; and slow outward stick travel that reaches the dash threshold after the tap window stays `Walk`. Dash still has priority over crouch, while crouch has priority over turn and diagonal down-walk input becomes `Squat` rather than `Walk`. `Squat` now consumes the same offensive priority slice before shield, jump, crouch-hold, or release, so down+A after the y tap window becomes down tilt instead of staying crouched, and down+B becomes down special. `Turn` now consumes its own state-local grounded action priority for side/down/up special, catch, and attacks before shield or jump, returns to `Wait` after `FighterProfile::standing_turn_total_frames`, intentionally ignores neutral B because the decomp callback does not call the neutral-special checker, and delays basic standing-turn visible facing until profile-owned `frames_to_change_direction_on_standing_turn` while offense before that flip uses `turn_facing_after`. `Guard` currently models the first no-item shield action slice: release enters `GuardOff`, down tap/C-stick down enters `EscapeN`, horizontal tap/C-stick side enters `EscapeF` or `EscapeB` by current facing, A/Z shield-grab enters `Catch`, and jump enters `KneeBend`; as an intentional Mole mechanic, held soft opposite stick starts a 5-frame shield turn that flips facing while staying in `Guard`, and a later roll uses that updated facing. The current `GuardOff` slice follows the source-backed path we can model without guard reflect state: spotdodge before jump, no roll or dash, and a provisional 15-frame return to `Wait`; its gated offensive branch depends on `mv.co.guard.x1C`, and exact animation duration still needs extraction. Platform shield drop, item throw, guard reflect, shield damage, and shield setoff are still future work. `Air` resolves fresh B through the common airborne B-special order as up, down, side, then neutral, then fresh digital L/R air dodge, then aerial attack from A or fresh C-stick edge, then air jump. Exact aerial callbacks, hitboxes, action-specific landing lag, and animation lengths are still future work. Diagonal A+stick tilt intent now resolves to side or up/down instead of falling through, using the decomp's angle-gate shape while exact `x20_radians` extraction remains future work. The exposed grounded action states return to `Wait` after the Captain Falcon total-frame counts already used by the test profile where those counts have been extracted, and the first IASA slice lets Falcon jab, up/down tilt, smashes, and neutral special resume grounded input priority at their IASA frames; exact side/up/down special animation lengths, charge, hitboxes, action-specific callbacks, and follow-ups are still future work. `Dash` preserves facing but applies acceleration from the current stick x, so an aged opposite-side input can produce moonwalk-like backward velocity without becoming a new dash, while a fresh opposite x tap during dash still enters `Turn`. After Falcon's 15-frame dash window, holding forward exits to `Run`, neutral exits to `Wait`, and holding the aged opposite side exits to analog `Walk`; walk now approaches the held analog target with acceleration/friction instead of snapping, so dash or moonwalk carry speed settles through physics. While running, neutral enters `RunBrake`, and full opposite stick enters `TurnRun` with traction instead of opposite acceleration on the entry tick; `RunBrake` can consume extracted `max_run_brake_frames` while the fallback profile leaves that field unset rather than guessing without DAT bytes. Grounded jump enters Falcon-style 4-frame jumpsquat instead of leaving the ground immediately, short-hop/full-hop selection comes from releasing the stored jump source during jumpsquat, jump out of shield uses the same `KneeBend` path without any shield-only hop-height rule, and a fresh digital L/R bottom-out can enter `EscapeAir` only after the fighter is airborne. Ordinary airborne contact enters `Landing`, whose duration now comes from `FighterProfile::normal_landing_lag_ticks` and the `ftCo_DatAttrs.normal_landing_lag` extractor path. `EscapeAir` now has a provisional `x334`-backed action phase that transitions to `FallSpecial` while airborne; landing during either state enters `LandingFallSpecial`, preserving horizontal slide under landing traction for a provisional 10-frame lag instead of snapping straight to idle. The exact air-dodge animation, `x334`, and `x344` values still need extraction. The motion state, state frame, turn target facing, shield-turn target/counter, jump source, and short-hop flag are included in replay checksums.

Jump takeoff now keeps the source-shaped horizontal velocity path: jumpsquat preserves grounded X velocity under traction, then takeoff combines carried ground speed with held main-stick X and clamps to the Falcon-profile jump horizontal max. Normal `Air` drift now consumes profile-owned `air_drift_stick_mul`, `aerial_drift_base`, `air_drift_max`, and `aerial_friction` fields instead of hard-coded simulator constants, so jump momentum persists through Melee-shaped physics and exact DAT-backed Falcon values can land through the profile extractor. Airborne grab/AirCatch is deliberately not a generic Falcon state: `ftCo_80095328` is held-item aerial throw/drop routing, while true `AirCatch` is Link/Young Link/Samus tether behavior.

Dash and run acceleration now use the same profile-owned `dash_run_acceleration_a`, `dash_run_acceleration_b`, and `dash_run_terminal_velocity` shape as the Melee helper `getAccelAndTarget`, instead of the old single Rust acceleration constant. Attack1 and AttackDash total frames/IASA now live in profile-owned `FighterActionFrames`, giving the Rust core a small checksum-covered seam for future extracted Falcon action data.

`Walk` now consumes a source-shaped IASA action ladder instead of only shield/jump/continued walk: catch/grab first, then B-specials in walk source order, then smashes, tilts, and jab. This prevents held walk input from swallowing fresh action inputs.

`KneeBend` now has the source-shaped jump-cancel IASA slice we can model without items: up special, catch/grab, then up smash, before short-hop release/takeoff. That keeps jump-cancel grab and jump-cancel up smash as first-class state transitions rather than special-case feel fixes.

Turn state now tracks Melee-shaped `has_turned`, one-frame `just_turned`, frames-to-turn, dash-out intent, and A/B latch data in rollback state. Fresh opposite smash-turn is handled as part of the dash check before crouch, does not flip facing on entry, can dash out on the actual turn frame, and can replay latched A/B on that frame.

`EscapeAir` physics now follows the source shape more closely during its action phase: it decays its own self-velocity and skips ordinary falling gravity until it becomes `FallSpecial`. The current decay percentage is still a provisional stand-in for `ftCommonData.escapeair_decay`.

The air-dodge vector now also follows the source shape: stick values inside `escapeair_deadzone` create no self-velocity, and non-deadzone input applies a fixed `escapeair_force` along the stick angle instead of scaling x/y independently. The current deadzone and force values are provisional until `PlCo.dat` extraction.

## Controllers

The launcher enables SDL's HIDAPI GameCube controller support before Pygame starts. With a WUP-028 adapter on WinUSB through Zadig, close any remapper software, unplug/replug the adapter if needed, then launch the game.

To see what the game can see, double-click `execs\Check Controllers.cmd`. For a live input readout, run:

```powershell
& '.\execs\Check Controllers.cmd' --watch
```

## Keyboard Controls

Player 1 can use keyboard input even while a controller is connected:

- Move: `WASD` or arrow keys
- Jump: `W`, up arrow, or `K`
- Attack: `Space` or `J`
- Special: `U`
- Shield: `L` or left shift
- Grab: `I`
- Pause: `P` or enter
- Quit: escape

## Legacy Pygame Smoke Test

This runs a short headless launch without opening a window:

```powershell
$env:SDL_VIDEODRIVER='dummy'; $env:MOLE_MAX_FRAMES='2'; .\.venv\Scripts\python RealMainFile.py
```
