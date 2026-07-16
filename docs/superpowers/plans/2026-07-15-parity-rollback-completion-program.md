# Decomp Parity And Rollback Completion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete shared decomp-derived gameplay architecture, truthful verification, deterministic rollback networking, and a higher-rate host scheduler without replay-specific gameplay behavior.

**Architecture:** The Rust core remains an authoritative deterministic 60 Hz simulation. Extracted immutable source data drives shared fighter, collision, and stage systems; replay supplies only recorded inputs. Rollback restores and resimulates whole 60 Hz frames, while an optional 120/240 Hz host cadence handles packet receipt, input capture, correction, and presentation.

**Tech Stack:** Rust workspace, Mole CLI, SDL3/WUP runtime, Slippi replay fixtures, Python extraction tools, baked Melee resources, UDP transport, deterministic rollback snapshots.

## Global Constraints

- Inspect the matching Melee decomp function, field, or table before changing gameplay parity behavior.
- Do not introduce replay-frame branches, replay state repair, tolerance widening, or continuation through classified disagreement.
- Keep global source rules global and character/stage data content-owned.
- Runtime code consumes compact baked artifacts and never raw DAT, ISO, JSON provenance, or decomp source.
- Authoritative gameplay advances only in whole 60 Hz frames; 120/240 Hz scheduling must not create fractional gameplay state.
- Every production behavior change follows a failing-test, observed-failure, source-proof, minimal-implementation, passing-test cycle.
- The 5,313-frame Falcon/Battlefield replay remains a mandatory regression after every gameplay/runtime task.
- Do not disable, silently skip, or weaken a failing test to make a gate green.

---

### Task 1: Freeze The Scoped Replay Milestone

**Files:**
- Modify: `.gitignore`
- Modify: `README.md`
- Create: `docs/release_notes/2026-07-15-captain-falcon-battlefield-full-replay-parity.md`
- Modify: `docs/worklogs/gameplay_parity_scratchpad.md`
- Modify: `docs/worklogs/2026-06-24-shield-replay-parity-milestone.md`

**Interfaces:**
- Consumes: current strict replay and generated-data evidence.
- Produces: a pushed checkpoint commit that later phases use as their comparison base.

- [ ] Review every changed and untracked path for temporary traces, secrets, raw source assets, and unrelated edits.
- [ ] Run formatting, compilation, Python, changed-crate, generated-data, CLI replay, and strict SDL replay gates.
- [ ] Record known broad-suite failures without claiming universal parity.
- [ ] Commit the complete scoped milestone and push the current branch.

### Task 2: Make Runtime And Core Contracts Truthful

**Files:**
- Modify: `crates/mole_runtime/tests/runtime_contract.rs`
- Modify: `crates/mole_runtime/src/slippi_diagnostic.rs`
- Modify: `crates/mole_core/tests/core_contract.rs`
- Create: `docs/research/contract-failure-classification.md`

**Interfaces:**
- Consumes: immutable replay fixture and source manifests.
- Produces: fixture loaders that return `Result`, explicit skip policy, and one source-owner classification for every failure.

- [ ] Add a failing contract proving a missing required replay fixture fails rather than returning successfully.
- [ ] Change required fixture helpers to return an error and propagate it through parity tests.
- [ ] Give every intentionally ignored diagnostic test an explicit reason or remove it.
- [ ] Re-run broad core/runtime suites and classify failures by shared source owner, separating stale fixtures from production gaps.
- [ ] Update stale fixtures only after the current decomp path proves the production result.

### Task 3: Translate The Shared AObj And Fighter Callback Scheduler

**Files:**
- Modify: `crates/mole_frame_data/src/lib.rs`
- Modify: `crates/mole_core/src/state.rs`
- Modify: `crates/mole_core/src/sim.rs`
- Modify: `crates/mole_runtime/src/lib.rs`
- Modify: `crates/mole_core/tests/core_contract.rs`
- Modify: `crates/mole_runtime/tests/runtime_contract.rs`

**Interfaces:**
- Produces: rollback-owned AObj playback state and one shared pre-callback animation/JObj evaluation phase.
- Consumes: extracted end frame, rewind frame, rate, loop flags, and source callback topology.

- [ ] Add focused failing tests for the nine scheduler-classified failures and callback replacement on transition ticks.
- [ ] Inspect and record the relevant HSD AObj and fighter callback decomp sequence.
- [ ] Add compact rollback/checksum-covered AObj playback state.
- [ ] Evaluate active and secondary skeletons before animation, input, physics, and collision callbacks in source order.
- [ ] Remove superseded action-specific frame offsets only when their replacement tests pass.
- [ ] Run core/runtime contracts and the strict replay gate.

### Task 4: Complete Shared Combat And Collision Lifecycle

**Files:**
- Modify: `crates/mole_core/src/collision.rs`
- Modify: `crates/mole_core/src/state.rs`
- Modify: `crates/mole_core/src/sim.rs`
- Modify: `crates/mole_core/tests/collision_contract.rs`
- Modify: `crates/mole_core/tests/core_contract.rs`
- Modify: `crates/mole_frame_data/src/lib.rs`

**Interfaces:**
- Produces: persistent source-shaped hit/hurt/grab capsule state and source callback routing.
- Consumes: live JObj output from Task 3 and extracted global/character combat values.

- [ ] Add failing tests for persistent hit endpoints, shield/lightshield, SDI, damage-floor, capture, grab, and throw groups.
- [ ] Trace each failing group to `ftColl`, `ftCo_Guard`, `ftCo_Damage`, or capture/throw decomp ownership.
- [ ] Persist previous world capsule endpoints and collision lifecycle flags in rollback state.
- [ ] Translate shared shield, damage-map-collision, capture, and throw callbacks without action or replay branches.
- [ ] Regenerate missing ECB/action-pose coverage through extraction.
- [ ] Run collision/core/runtime contracts and the strict replay gate.

### Task 5: Reconcile Tests And Diagnostics

**Files:**
- Modify: `crates/mole_core/tests/core_contract.rs`
- Modify: `crates/mole_runtime/tests/runtime_contract.rs`
- Modify: `crates/mole_runtime/src/slippi_diagnostic.rs`
- Modify: `crates/mole_core/src/sim.rs`

**Interfaces:**
- Consumes: completed shared scheduler and collision architecture.
- Produces: green non-ignored core/runtime suites and diagnostics outside release hot paths.

- [ ] Replace obsolete milli/profile fixtures with source-float, CollData, action-descriptor, and pose fixtures.
- [ ] Delete bespoke ignored replay probes after equivalent Mole CLI diagnostics exist.
- [ ] Move verbose collision tracing behind a diagnostics feature with no release-path formatting cost.
- [ ] Require all non-ignored core/runtime contracts to pass.

### Task 6: Finish The Agnostic Compact Extraction Pipeline

**Files:**
- Modify: `crates/mole_cli/src/frame_data.rs`
- Modify: `crates/mole_cli/src/runtime_data.rs`
- Modify: `tools/generate_falcon_ecb_rust.py`
- Modify: `tools/extract_melee_resources.py`
- Modify: `crates/mole_core/src/generated/falcon_ecb.rs`
- Modify: `tests/test_extract_melee_resources.py`
- Modify: `tests/test_generate_falcon_ecb_rust.py`

**Interfaces:**
- Produces: generic character pose/collision bundles and compact indexed runtime lookup.
- Consumes: raw source assets only inside extraction tools.

- [ ] Add failing size/reproducibility tests for the compact ECB replacement.
- [ ] Define one generic bundle schema for action/FObj, skeleton, part, ECB-joint, capsule, and costume data.
- [ ] Replace expanded Falcon ECB source with compact lossless binary/static data and indexed runtime decoding.
- [ ] Add Marth extraction and second-stage extraction smoke contracts without gameplay branches.
- [ ] Prove byte-for-byte reproducibility and no raw-source runtime access.

### Task 7: Make Rollback Schema And Protocol Self-Checking

**Files:**
- Modify: `crates/mole_core/src/state.rs`
- Modify: `crates/mole_rollback/src/lib.rs`
- Modify: `crates/mole_rollback/tests/rollback_contract.rs`
- Modify: `crates/mole_transport/src/lib.rs`
- Modify: `crates/mole_transport/tests/transport_contract.rs`
- Modify: `crates/mole_runtime/src/main.rs`
- Modify: `crates/mole_runtime/src/lib.rs`

**Interfaces:**
- Produces: one authoritative snapshot/hash/restore declaration, bounded histories, explicit checksum frames, and compatibility fingerprints.

- [ ] Add failing exhaustive snapshot round-trip coverage with every authoritative field non-default.
- [ ] Generate snapshot capture, restore, and checksum traversal from one field declaration.
- [ ] Add failing long-session coverage for bounded transport inbox history.
- [ ] Add protocol fields for build/artifact fingerprint and `checksum_frame` with versioned decoding tests.
- [ ] Retain and compare equivalent-frame local/remote checksums; surface mismatch as a hard session error.
- [ ] Run rollback, transport, runtime, and strict replay gates.

### Task 8: Remove Rollback-Critical Allocations And Scans

**Files:**
- Modify: `crates/mole_runtime/src/lib.rs`
- Modify: `crates/mole_core/src/state.rs`
- Modify: `crates/mole_rollback/src/lib.rs`
- Modify: `crates/mole_runtime/tests/runtime_contract.rs`

**Interfaces:**
- Produces: a production simulation path that does not build discarded diagnostic vectors or linearly scan immutable action data.

- [ ] Add release benchmarks for ordinary simulation and 1-8-frame collision-heavy rollback.
- [ ] Split diagnostic collision output from the allocation-minimal production result.
- [ ] Replace action-cache scans with immutable indexed lookup.
- [ ] Avoid cloning unchanged combat logs during snapshot/resimulation.
- [ ] Record p50/p95/p99 time, allocation count, and retained memory.

### Task 9: Implement The 60/120/240 Hz Host Scheduler

**Files:**
- Modify: `crates/mole_runtime/src/main.rs`
- Modify: `crates/mole_runtime/src/lib.rs`
- Modify: `crates/mole_runtime/tests/runtime_contract.rs`
- Modify: `docs/architecture/native-rust-rollback-architecture.md`

**Interfaces:**
- Produces: configurable host cadence that polls network/input and initiates rollback between whole 60 Hz gameplay ticks.

- [ ] Add failing deterministic tests that run identical inputs at 60, 120, and 240 Hz host cadence.
- [ ] Introduce explicit host-pass and simulation-frame clocks with integer cadence ratios.
- [ ] Poll packets and capture input on host passes while advancing gameplay only on 60 Hz boundaries.
- [ ] Permit late-input rollback/resimulation before the next presentation boundary.
- [ ] Assert identical frame checksums, replay output, and final state across all host rates.

### Task 10: Validate Production Online Rollback

**Files:**
- Modify: `crates/mole_transport/tests/transport_contract.rs`
- Modify: `crates/mole_runtime/tests/runtime_contract.rs`
- Create: `docs/research/rollback-performance-report.md`

**Interfaces:**
- Consumes: Tasks 7-9.
- Produces: two-process deterministic, adverse-network, long-session, and latency evidence.

- [ ] Add deterministic packet delay, reorder, duplicate, drop, and correction scenarios across the full rollback window.
- [ ] Run two fresh processes and compare every historical checksum after convergence.
- [ ] Exercise repeated connection, disconnect, and long-session memory behavior.
- [ ] Benchmark all scheduler rates and rollback depths on the reference machine.
- [ ] Record deadline misses and performance distributions without weakening correctness gates.

### Task 11: Final Release And GitHub Gate

**Files:**
- Modify: `README.md`
- Modify: `docs/worklogs/gameplay_parity_scratchpad.md`
- Create: `docs/release_notes/2026-07-15-parity-rollback-completion.md`

**Interfaces:**
- Consumes: all previous task evidence.
- Produces: reviewed commits, clean generated artifacts, and a pushed GitHub-ready branch.

- [ ] Run full Rust workspace, all-feature runtime, Python, generated-data, strict CLI replay, strict SDL replay, multi-process determinism, and rollback performance gates.
- [ ] Run formatting and diff validation with no unexplained warnings or ignored tests.
- [ ] Dispatch an independent whole-branch review and resolve every critical/important finding.
- [ ] Update milestone documentation with exact scope, commands, counts, and measured performance.
- [ ] Commit, push, and report the branch and remaining explicitly out-of-scope work.
