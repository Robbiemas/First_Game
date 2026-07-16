# Parity Rollback Completion Milestone

Date: 2026-07-16

## Scope

This checkpoint hardens the native Rust rollback and Friend Connect layers on
top of the scoped Captain Falcon/Battlefield replay milestone. It does not claim
universal fighter, stage, or common-action parity. Replay remains ordinary game
inputs executed by the same collision-aware gameplay engine.

## Architecture

- Authoritative gameplay and rollback resimulation remain whole 60 Hz frames.
- Friend Connect independently resolves 60/120/180/240 Hz host passes for UDP,
  controller capture, correction, and presentation opportunities.
- The production rollback window is Slippi's seven frames. Snapshot, input,
  packet, and checksum histories are bounded.
- Packets carry delayed input frame and finalized checksum frame separately.
  Predicted checksums are never presented as finalized agreement.
- Protocol, authoritative Rust source build, baked artifact, and shared-room
  compatibility is checked before current-session packet admission.
- Exact finalized-frame mismatch, conflicting checksum claims, expired checksum
  validation, and incompatible sessions are hard errors. No checksum path edits,
  repairs, or resynchronizes gameplay state.
- Resimulation requires the complete recorded interval and uses the configured
  production step function. Missing history is rejected rather than replaced by
  invented neutral input.

## Performance Work

- Immutable runtime actions use indexed lookup instead of repeated linear scans.
- Rollback snapshots share unchanged hit-victim history copy-on-write.
- Reproducible release measurements are in
  `docs/research/rollback-performance-report.md`.
- On the i7-6700K reference machine, ordinary simulation p99 was 2.412 ms.
  Seven-frame rollback p50/p95/p99 was 5.695/13.014/23.566 ms. The live cadence
  resolver uses actual host deadline misses; this isolated harness is not an
  end-to-end cadence measurement or a low-end 240 Hz guarantee.

## Verification

Passing gates:

- `cargo fmt --all -- --check`
- `git diff --check`
- `cargo check --workspace`
- `cargo check -p mole_runtime --features "sdl wup"`
- `mole_transport`: 28 tests
- `mole_rollback`: 17 tests
- `mole_core --lib`: 84 tests
- `mole_core collision_contract`: 11 tests
- `mole_runtime --lib`: 44 passed, 2 explicitly ignored diagnostics
- host cadence: 4 tests
- cadence resolver: 3 tests
- adverse rollback network: 2 tests
- focused Friend Connect: 31 tests
- strict `slippi_match_start_full_fixture_has_no_engine_divergence`: all 5,313
  replay frames, no classified engine divergence
- independent final review: no blocking P0/P1 findings after packet-horizon,
  compatibility, checksum, finalization, and legacy-wire remediations

Known verification limits:

- Broad `mole_core core_contract` remains 580 passed, 41 failed, 2 ignored. The
  same classified shared-system/stale-fixture groups predate this rollback work.
- The monolithic `mole_runtime runtime_contract` target exceeded a fresh
  15-minute timeout. Its mandatory strict 5,313-frame gate and changed runtime
  slices pass independently; the entire target is not claimed green.
- Full p4/p6 callback extraction and universal fighter/stage parity remain open.
