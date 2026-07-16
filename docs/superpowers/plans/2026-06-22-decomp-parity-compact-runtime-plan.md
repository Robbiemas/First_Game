# Decomp-Parity Compact Runtime Plan

## Summary

Build toward true Melee decomp parity by making decomp-shaped source dataflow the engine contract while preserving the runtime pillars:

- Parity: decomp is the primary source of truth; Slippi is a secondary witness.
- Speed: normal play consumes compact preloaded runtime artifacts only.
- Size: runtime data stays in the same order of magnitude as gameplay source data unless a benchmark proves a deliberate expansion is worth it.
- Editability: devtool and CLI customization use typed overlays and resolved artifacts, not mutation of extracted baselines.

Target dataflow:

```text
decomp / ISO source truth
  -> lossless CLI extraction
  -> editable typed override layer
  -> compact baked runtime artifacts
  -> decomp-shaped Rust simulation
  -> replay, rollback, and performance validation
```

## Architectural Rules

- Extracted source data is immutable baseline truth.
- Source floats stay floats through extraction, override resolution, baking, and runtime.
- Large per-frame JSON/sample outputs are debug or parity artifacts unless explicitly proven and documented as runtime-critical.
- Runtime must not parse JSON, DAT, decomp files, editor schemas, or the filesystem during normal play.
- Rollback snapshots store authoritative simulation state only, never source tables, provenance, strings, editor metadata, or debug data.
- Replay divergences must be assigned to a decomp phase before a parity change is made.
- Devtool ECB editing must be indirect: edit the decomp-shaped source inputs, pose/rig/animation data, or typed overrides that the decomp-shaped ECB evaluator consumes. Do not make hand-authored gameplay ECB boxes the source of truth unless the decomp proves that source exists.

## Runtime Artifact Contract

Runtime-facing data should be compact, baked, and preloadable:

- Source action metadata.
- FigaTree/action animation bundles.
- ECB source setup and evaluated ECB outputs.
- Stage collision lines and topology.
- Source action callbacks/events.

Debug and middleware artifacts may remain checked in temporarily, but they must be classified away from normal runtime consumption.

The first guardrail is:

```powershell
cargo run -p mole_cli -- runtime-data size-report --json
```

This report classifies runtime, middleware, debug/sample, and legacy expanded artifacts. `crates/mole_core/src/generated/falcon_ecb.rs` is currently expected to remain flagged until the live source-backed ECB evaluator replaces the expanded table path.

## Phase 1: Baseline Audit And Guardrails

- Inventory extracted/generated runtime artifacts and record source size versus runtime size.
- Add a size-report CLI command that flags large expansions.
- Add tests that fail if runtime begins consuming debug JSON/sample caches.
- Mark large per-frame artifacts as debug-only in docs and code comments.
- Produce a parity gap ledger for ECB, collision, stage data, and action-state execution.

Current status:

- `runtime-data size-report` exists and is tested.
- Debug/sample artifacts are classified outside the runtime contract.
- Legacy expanded Falcon ECB Rust is flagged as a runtime-size warning.
- Remaining Phase 1 work: expand the parity gap ledger and connect it to CLI/devtool inspection.

## Phase 2: ECB Runtime Parity Foundation

Make live ECB evaluation the primary runtime path. Implement only from decomp proof, starting with `mpColl_LoadECB_JObj` and adjacent lifecycle code:

- Clear flag handling.
- Source joint world sampling.
- `+/-2.0F` expansion.
- `x128/x12C` min extents.
- Flag-specific side/bottom/top clamps.
- Side midpoint.
- Sanitize pass.
- Bottom lock behavior.
- Interpolation.

Tests must compare live evaluator results against decomp-derived sample outputs for representative actions and flags. Once live evaluation matches, demote or remove baked per-frame ECB fallback.

## Phase 3: Stage And Collision Parity

Replace fixed Battlefield assumptions with extracted `MapCollData`-style topology:

- Floors.
- Left and right walls.
- Ceilings.
- Ledges.
- Pass-through platforms.
- Floor skip.
- Line connectivity.
- Surface attributes and friction.

Battlefield is validated first. Additional stages wait until the format and collision loops are proven.

## Phase 4: Fighter State Architecture

Move runtime behavior toward decomp phases instead of Rust convenience shortcuts:

- Fighter tick/update order.
- State transition tables and callback routing.
- Throws/capture.
- Tech/passive.
- Wall and ceiling damage callbacks.
- Downed states.
- Ledge states.
- Shield/guard details.
- UCF overlay behavior.

Earliest proven decomp-phase mismatch drives replay divergence work.

## Phase 5: Rollback And Performance Hardening

Benchmark and enforce deterministic, compact simulation:

- 60 Hz single-step scenarios.
- 240 simulated steps/sec target.
- Rollback resim windows of 4, 8, 12, and 16 frames.
- Two-player Battlefield combat stress case.
- Hot-path allocation checks where feasible.
- Replay/resim checksum tests.

Optimization happens after measurement:

- Cache evaluated poses/ECB only if measured.
- Flatten lookup tables only if measured.
- Avoid dynamic dispatch/string lookup in hot paths.
- Predecode compact binary artifacts at preload time.

## Validation Policy

Every parity change needs evidence:

- Decomp function/table/field reference.
- Rust mapping or explicit known gap.
- Focused unit or contract test.
- Replay/trace witness only after the decomp phase is identified.
- Performance/size report when runtime artifacts or hot paths change.
