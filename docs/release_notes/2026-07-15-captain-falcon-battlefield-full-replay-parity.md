# Captain Falcon/Battlefield Full-Replay Parity - 2026-07-15

## Scope

This checkpoint completes strict end-to-end playback of the current
Captain Falcon versus Captain Falcon Battlefield Slippi fixture. The replay
boots its recorded settings and controller inputs into the ordinary gameplay
engine. Expected Slippi state is diagnostic data only: the runtime neither
repairs gameplay from the replay nor continues past a classified disagreement.

This is not yet universal Melee parity. Other fighters, stages, and common
action families still require extraction and source-faithful implementation.

## Evidence

- `cargo run -q -p mole_cli -- replay check --inputs
  debug/slippi/Game_20260530T214929.inputs.json --frames 6000 --json` compared
  all 5,313 source frames with zero classified divergence, state mismatch,
  position drift, or unsupported state.
- The strict SDL runtime reached core frame 6000 under dummy video/audio drivers
  and did not create a divergence log.
- The user separately watched the replay and accepted the completed visual run.
- Runtime source data regenerated from the source manifest without stale or
  missing artifacts. The four compact runtime artifacts total 14,582,541 bytes.

## Architecture

- Replay and live play share world construction, command-script execution,
  collision, motion, and physics. There are no replay-frame gameplay branches
  or state resynchronization paths.
- Raw action flags, command events, collision state, TransN handling, and
  character values flow through generic extraction/runtime structures. Global
  rules remain global; Falcon data remains character-owned.
- Mutable authoritative fighter state added during parity work is included in
  rollback snapshots and checksums. A regression restores every recently added
  field, including source collision, ledge, fall blend, Falcon SpecialHi, and
  lightshield state.
- Rollback input history is pruned to the recoverable snapshot window, including
  stale-correction attempts, so long sessions do not grow that history without
  bound.

The authoritative simulation cadence remains 60 Hz. The host/network loop may
run at 120 or 240 Hz to receive packets and initiate rollback earlier, but it
must resimulate whole deterministic 60 Hz gameplay frames. Fractional fighter
physics are not part of this milestone, and a production higher-rate scheduler
has not yet been performance-qualified.

## Verification

The changed surfaces are green:

- `cargo test -p mole_input`: 16 passed.
- `cargo test -p mole_rollback`: 12 passed.
- `cargo test -p mole_transport`: 21 passed.
- `cargo test -p mole_cli`: 106 passed.
- `cargo test -p mole_runtime --lib`: 41 passed, 2 ignored.
- Full Python suite: 262 passed.
- `cargo check --workspace`: passed.
- State-graph/value comparison reports zero current value mismatches.

The broad `mole_core` contract suite is not green: 568 passed, 46 failed, and
2 were ignored at this checkpoint. The failures include stale legacy fixtures
and real unimplemented source architecture, chiefly the shared AObj callback
scheduler and additional combat/collision behavior. They are not hidden by this
release note and prevent describing the repository as universally complete.
The large `mole_runtime` contract target also remains a known non-green baseline
(last classified at 221 passed and 84 failed); only its 43-test library target
is part of the green matrix above.

`mole generated check` reports zero stale, missing-input, or missing-output
groups. Five groups are reported non-OK only because their regenerated outputs
are intentionally dirty in this uncommitted milestone diff; that signal should
be clean after the checkpoint is committed.

## Follow-Up Boundaries

- Complete shared source architecture before expanding parity claims to other
  fighters and stages; do not add fixture-specific gameplay fixes.
- Add a netplay build/artifact fingerprint and an unambiguous remote checksum
  frame contract, then compare received checksums in-session.
- Bound transport packet-inbox history to the same rollback horizon and profile
  rollback/resimulation allocations before calling online performance final.
- Implement and benchmark the optional 120/240 Hz host/network scheduler while
  preserving 60 Hz authoritative simulation.
- Replace the legacy expanded Falcon ECB Rust table with an equally lossless,
  compact runtime representation. It remains approximately 8 MB and is the
  largest known generated-code size debt.

This milestone is suitable for a scoped checkpoint commit and GitHub push once
the final strict replay and formatting checks are repeated on the exact diff.
It is not a universal parity release or final rollback-netcode performance signoff.

## 2026-07-16 Scheduler Completion Addendum

The strict fixture is now enforced directly by
`slippi_match_start_full_fixture_has_no_engine_divergence`, which runs all 5,313
frames and rejects the first classified engine disagreement. Global fighter
priority ordering now covers hitlag expiry, animation callbacks, and the
migrated Turn/AttackAir input callbacks used by this fixture. The former source-frame
2583/2586 diagnostic expectations were removed only after this strict gate
proved those rows aligned.

The full p4/p6 per-player monolith has not yet been extracted into global
phases for every action family. That remaining scheduler work is included in
the broad non-green boundary and is not implied by this scoped checkpoint.

The latest broad core result is 580 passed, 41 failed, and 2 ignored. Those
remaining contracts continue to bound the claim: this branch is ready as the
Falcon/Battlefield replay scheduler milestone, not as universal Melee parity or
finished online rollback qualification.
