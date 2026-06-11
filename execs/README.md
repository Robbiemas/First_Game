# Execs

Double-click these Windows launchers from this folder.

Normal development now targets the Rust runtime and crates. Deprecated Pygame,
generic controller, and standalone smoke launchers live in `depreciated/`.

- `Check WUP Native.cmd`: checks the WUP-028 adapter directly through WinUSB/libusb.
- `Monitor WUP Native.cmd`: opens the native WUP input display with main stick, C-stick, D-pad, split L/R analog triggers, and split L/R digital trigger clicks.
- `Run Rust Runtime.cmd`: runs the deterministic Rust core smoke test.
- `Run SDL3 Runtime.cmd`: opens the optimized native SDL3/WUP local game window and starts gameplay immediately; it does not require Friend Connect or a secondary client.
- `Run SDL3 Runtime Vanilla No UCF.cmd`: opens the same optimized local SDL3/WUP runtime with UCF disabled, leaving the WUP/native pre-UCF path exposed for controller feel testing.
- `Record Native Replay.cmd`: records a deterministic Rust runtime replay under `debug/replays/`.
- `Setup SDL3.cmd`: helper used by the SDL3 launchers.
- `Open Dev Tool.cmd`: opens the Rust Mole Game Dev Tool without launching the game.
- `Build Friend Playtest Package.cmd`: builds a self-contained Windows playtest folder,
  zip, and one-file bootstrap exe under `dist/`, then updates
  `playtest/MoleGame-FriendPlaytest.exe` and
  `playtest/MoleGame-LocalInternetPlaytest.exe` for playtesting.
  Agents should prefer `cargo run -p mole_cli -- package friend-playtest --json`;
  it runs this packaging path and verifies the one-file exe extracts and starts
  without depending on the source repository's asset path.
  `cargo run -p mole_cli -- package local-internet-playtest --json` is the
  explicit command for refreshing the secondary solo internet launcher.

Friend Connect singles uses lobby role for player ownership: the code-owner is
P1 and Start owner, while the player who enters that code is P2 and waits for
Start. Local controllers are treated as inactive at launch. The first connected
local WUP/GameCube controller that produces non-neutral gameplay input latches
as that machine's active controller; extra local controllers are ignored for
this singles build and left for a future local-doubles pass. For a read-only
summary of this contract plus the current package artifact status, run
`cargo run -p mole_cli -- friend-connect status --json`.

Friend Connect gameplay packets now use match frame `0` as the shared gameplay
epoch after Start. The runtime applies Slippi-style online delay: physical
input sampled on match frame `F` is scheduled and transmitted immediately for
game frame `F + delay`, with default delay `2`. It retransmits the newest `8`
future-stamped input frames each frame until ACKs allow old packets to drop.
The delay is manually tunable with `--netplay-delay N` when launching the
runtime directly.

The packaged playtest also includes a solo internet-path harness:
`Run Solo Internet Host.cmd` starts the normal visible P1 host, and
`Run Headless Internet Peer.cmd` prompts for that host code before starting a
headless P2. This still uses Supabase setup and direct UDP gameplay packets.
Both roles write compact JSONL diagnostics under `logs\netplay`, capped at
5 MB per role/session and summarized every 60 gameplay frames.
The one-file `MoleGame-LocalInternetPlaytest.exe` starts a visible local
internet harness directly by generating the local code, launching a visible P1
host, and launching a visible P2 peer. Supabase still handles setup, while
same-machine gameplay packets use explicit loopback UDP ports
`127.0.0.1:41001` and `127.0.0.1:41002` so packet flow is visible without router
NAT hairpin ambiguity. Each Friend Connect status panel shows `CPU nHZ` headroom
before the deterministic 60 Hz cap wait.

Deprecated launchers kept for old-reference/manual recovery work:

- `depreciated/Launch Mole Game.cmd`
- `depreciated/Launch Mole Game Debug.cmd`
- `depreciated/Live Test Session.cmd`
- `depreciated/Check Controllers.cmd`
- `depreciated/Check SDL3 Inputs.cmd`
- `depreciated/Run WUP Native Runtime.cmd`
- `depreciated/Open GameCube Calibration.cmd`
