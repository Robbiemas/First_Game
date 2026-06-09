# Headless Internet Peer Design

Date: 2026-06-05

## Goal

Add a parallel Friend Connect test mode that lets one machine run the normal
visible game client as P1 and a second local internet peer as P2. The one-click
launcher now defaults to a visible P2 peer so both simulations can be inspected.
The explicit headless P2 mode remains available as a diagnostics-only option.
Both modes must use the same Supabase setup path, direct UDP gameplay packet
path, Slippi-style future-frame input delay buffer, ACK repair window, and
rollback confirmation path as a real two-machine Friend Connect session.

## Scope

This is a diagnostics and solo playtest harness. It does not replace real P2P
Friend Connect, does not send gameplay input through Supabase, and does not
change deterministic 60 Hz gameplay logic after inputs are committed.

The first implementation supports:

- A visible joiner process that connects to a host room code.
- A local one-click launcher that starts the visible host and visible peer with
  a generated shared code.
- An explicit headless joiner command for lower-noise diagnostics.
- Scripted neutral P2 input by default, with the same packet timing and repair
  behavior as a real peer.
- Compact per-role logs that show setup, endpoint, packet health, rollback, and
  checksum anomalies without dumping every frame.

## Architecture

The visible host and visible peer both use the normal `--friend-connect --play`
runtime. The host can be launched with
`--friend-code CODE --auto-start --friend-local-udp 127.0.0.1:41001`; the
visible peer can be launched with
`--friend-code PEER --connect-code CODE --friend-local-udp 127.0.0.1:41002`.
Supabase still handles setup, while the same-machine harness uses explicit
loopback UDP for gameplay packets to avoid NAT hairpin ambiguity. The headless
process remains a separate diagnostics mode,
`--friend-connect-headless-peer --connect-code CODE`, using a generated local
peer id, Supabase setup signaling, a direct UDP socket, and `RollbackSession`.
Friend Connect gameplay must use
`SlippiInputDelayBuffer`: physical input sampled on match frame `F` is scheduled
and transmitted immediately for game frame `F + delay`.

Both roles log through a bounded writer. Logs are JSON Lines in `logs/netplay/`
with one file per role/session. Normal frame health is summarized every 60
frames. Important anomalies are logged immediately. The writer enforces a small
maximum file size and stops logging once the cap is reached instead of growing
without bound.

## Data Flow

1. The visible host opens the existing Friend Connect lobby and shows its code.
2. The headless peer receives that code through CLI args or a generated launcher.
3. The headless peer joins the host room through Supabase setup signaling and
   publishes its UDP endpoint.
4. The host and headless peer exchange direct UDP `InputPacket`s.
5. The host still starts the match. The headless peer listens for `match_start`.
6. Once started, both peers use match frame `0`, default netplay delay `2`,
   `F + delay` packet stamping, and recent packet retransmission.
7. The visible peer feeds its local controller if available, or neutral input
   when no WUP adapter is available because the host owns it.
8. The headless peer, when explicitly launched, feeds deterministic
   neutral/scripted input for P2 and consumes P1 packets for rollback
   prediction/correction.

## Logging

The logger records:

- session start: role, room code, peer id, delay, package/runtime version fields
- setup: Supabase connect/join, endpoint selection, peer endpoint received
- frame summary every 60 frames: frame, sent, received, duplicate, missing,
  rollback corrections, last RTT, checksums
- anomalies: missing remote input bursts, stale packet correction miss,
  checksum mismatch, socket/signaling errors, crash/panic if caught by caller

The logger must not record every input packet by default. The initial cap is
5 MB per role log file. If the cap is reached, it emits one final
`log_cap_reached` event and disables further writes for that file.

## Error Handling

NAT hairpin failure is expected on some routers when both peers run on one PC.
The headless mode must report this as an endpoint/UDP reachability failure, not
silently fall back to in-memory loopback. Local LAN fallback may appear only as
the existing STUN fallback behavior and must be visible in logs.

WUP missing or busy remains nonfatal for the visible host. The headless peer
does not require WUP.

## Testing

Focused tests should cover:

- CLI parsing for headless peer args.
- Role/log configuration defaults.
- Bounded logger cap behavior.
- Headless input defaults to deterministic neutral input.
- Status/package output advertises solo internet test mode.

Manual verification should rebuild the playtest package and confirm the one-file
EXE still extracts all required files.
