# Execs

Double-click these Windows launchers from this folder.

Normal development now targets the Rust runtime and crates. Deprecated Pygame,
generic controller, and standalone smoke launchers live in `depreciated/`.

- `Check WUP Native.cmd`: checks the WUP-028 adapter directly through WinUSB/libusb.
- `Monitor WUP Native.cmd`: opens the native WUP input display with main stick, C-stick, D-pad, split L/R analog triggers, and split L/R digital trigger clicks.
- `Run Rust Runtime.cmd`: runs the deterministic Rust core smoke test.
- `Run SDL3 Runtime.cmd`: opens the native SDL3/WUP game window and runs until the window is closed.
- `Run SDL3 Runtime Vanilla No UCF.cmd`: opens the same native SDL3/WUP game window with UCF disabled, leaving the WUP/native pre-UCF path exposed for controller feel testing.
- `Record Native Replay.cmd`: records a deterministic Rust runtime replay under `debug/replays/`.
- `Setup SDL3.cmd`: helper used by the SDL3 launchers.
- `Open State Graphs.cmd`: opens the Mole Game Dev Tool without launching the game.
- `Build Friend Playtest Package.cmd`: builds a minimal Windows playtest folder,
  zip, and one-file bootstrap exe under `dist/`, then updates
  `playtest/MoleGame-FriendPlaytest.exe` for README download links.

Deprecated launchers kept for old-reference/manual recovery work:

- `depreciated/Launch Mole Game.cmd`
- `depreciated/Launch Mole Game Debug.cmd`
- `depreciated/Live Test Session.cmd`
- `depreciated/Check Controllers.cmd`
- `depreciated/Check SDL3 Inputs.cmd`
- `depreciated/Run WUP Native Runtime.cmd`
- `depreciated/Open GameCube Calibration.cmd`
