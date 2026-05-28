# Supabase Signaling Notes

Date checked: 2026-05-28.

Supabase Realtime is acceptable for lobby and connection setup work. It should
not carry deterministic 60 Hz gameplay input.

## Current Official Limits

Official Supabase Realtime limits currently list the Free plan at:

- 200 concurrent connections.
- 100 messages per second.
- 100 channel joins per second.
- 20 presence messages per second.
- 256 KB broadcast payload size.

The same docs say exceeding message throughput can disconnect clients until
traffic falls back below the project limit.

Pricing docs also count Realtime messages and peak connections as billable
usage above plan quota. Reports docs recommend watching connection counts,
broadcast events, presence events, channel joins, payload size, errors, and
response speed.

Sources:

- https://supabase.com/docs/guides/realtime/limits
- https://supabase.com/docs/guides/realtime/pricing
- https://supabase.com/docs/guides/realtime/reports

## Project Usage Rule

Allowed Supabase purposes:

- Presence.
- Matchmaking.
- Room codes.
- Setup messages.
- Direct UDP endpoint exchange.
- Future WebRTC offer, answer, and ICE candidate exchange.

Disallowed Supabase purposes:

- Frame-indexed gameplay input.
- Simulation checksums.
- Rollback prediction data.
- Any 60 Hz gameplay transport.

## Why Gameplay Input Stays Off Supabase

A naive two-player stream at 60 Hz is 120 input events per second before
acknowledgements, resends, matchmaking traffic, or other players are counted.
That already exceeds the current Free plan Realtime message-per-second limit.

Even on higher tiers, server-mediated Realtime is the wrong latency and
ownership model for this game. The intended gameplay path is still:

1. Each peer owns local input.
2. Peers exchange frame-indexed input over direct UDP first.
3. Rollback prediction and resimulation own temporary disagreement.
4. Supabase only helps peers find each other and exchange setup data.

## Failure Modes To Expect

- Room creation or join messages may be delayed by Realtime service conditions.
- Channel joins can be refused when the project exceeds channel or connection
  limits.
- Message throughput can trigger disconnects.
- Presence is useful for lobby state but not a reliable gameplay clock.
- WebRTC setup through Supabase may still fail if NAT traversal requires TURN.

## Implementation Boundary

`mole_signaling` owns setup message shapes and Supabase usage policy.
`mole_transport` owns gameplay packet transport.
`mole_rollback` owns prediction and resimulation.
`mole_core` owns deterministic simulation.
