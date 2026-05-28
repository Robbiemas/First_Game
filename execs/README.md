# Execs

Double-click these Windows launchers from this folder.

- `Launch Mole Game.cmd`: opens the original Pygame prototype and starts the native WUP input bridge when available.
- `Launch Mole Game Debug.cmd`: opens the game with the input/state overlay enabled and writes frame logs under `logs/`.
- `Live Test Session.cmd`: opens the game in debug/log mode and opens the side-by-side Melee/Mole state graph viewer. It watches project files while the game is open, rebuilds the WUP helper after Rust edits, and exits when the game closes. Only one live session can run at a time.
- `Open State Graphs.cmd`: opens the side-by-side Melee reference vs Mole current state transition graph without launching the game.
- `Check Controllers.cmd`: prints what the Python/Pygame path can see. Add `--watch` from a terminal for live updates.
- `Check SDL3 Inputs.cmd`: lists devices visible through the SDL3 Rust path.
- `Check WUP Native.cmd`: checks the WUP-028 adapter directly through WinUSB/libusb.
- `Monitor WUP Native.cmd`: opens the native WUP input display with main stick, C-stick, D-pad, split L/R analog triggers, and split L/R digital trigger clicks.
- `Open GameCube Calibration.cmd`: deprecated legacy endpoint tool. Normal native GameCube input learns its origin automatically at plug-in/launch, and `X + Y + Start` held for about three seconds recenters it; current gameplay does not use manual endpoint calibration.
- `Run Rust Runtime.cmd`: runs the deterministic Rust core smoke test.
- `Run SDL3 Runtime.cmd`: runs the SDL3 runtime smoke loop.
- `Run WUP Native Runtime.cmd`: runs the native WUP runtime smoke loop.
- `Record Native Replay.cmd`: records a deterministic Rust runtime replay under `debug/replays/`.
- `Setup SDL3.cmd`: helper used by the SDL3 launchers.
