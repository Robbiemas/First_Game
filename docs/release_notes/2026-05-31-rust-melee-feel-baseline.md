# Rust Melee Feel Baseline

Date: 2026-05-31

This checkpoint saves the first human-confirmed native Rust movement baseline:
the SDL3/WUP runtime now feels close enough to Melee that grounded dash/dash
dance/wavedash control can be used as a forward development baseline instead of
the legacy Pygame prototype.

## Launchers

- `execs\Run SDL3 Runtime.cmd`: normal native SDL3/WUP playtest, UCF enabled.
- `execs\Run SDL3 Runtime Vanilla No UCF.cmd`: same runtime with adapter-owned
  UCF disabled for pre-UCF GameCube input testing.
- `execs\Monitor WUP Native.cmd`: direct WUP input visualizer.
- `execs\Open State Graphs.cmd`: Mole Game Dev Tool with state graphs, parity
  ledger, and Slippi replay diagnostics.

## What Is Preserved

- Rust core remains authoritative for deterministic 60 Hz simulation,
  rollback-owned inputs, motion states, physics, ECB, snapshots, checksums, and
  replay diagnostics.
- Pygame remains legacy/reference only.
- UCF remains outside `mole_core` in adapter/input preprocessing.
- The WUP path preserves raw GameCube bytes, captures origin at the input
  boundary, applies Melee/HSD stick clamp and scale before UCF, and collapses
  subframe USB reports off the gameplay thread.
- The current compact core input bridge encodes Melee's HSD-normalized
  `stick / 80.0f` value into signed `-127..127` axes until the core moves fully
  to source-shaped fighter floats.

## Key Implementation Slices

- Source-shaped grounded locomotion for the current Falcon-like test character:
  Dash, Run, RunBrake, TurnRun, Walk, Turn, and Wait carry decomp-backed motion
  variables, callback ordering, and value data more faithfully.
- Falcon/common extracted value sheets are synchronized with Rust data:
  `103/103` current value rows match.
- Falcon ECB mappings cover the sampled motion-state set with no missing sampled
  mappings.
- Slippi diagnostics can report state mismatch, position drift, source replay
  frame, Rust core frame, and trace windows.
- Runtime input traces expose raw WUP, origin-adjusted, HSD/native, UCF, final
  core input, and resulting state/velocity facts.
- Native WUP reads run on a background worker and the 60 Hz gameplay tick drains
  the capture queue without blocking on USB.

## Verification At Save Point

- `cargo test --workspace`: passed.
- Python dev-tool suite:
  `tests\test_value_sheets.py`, `tests\test_state_graph_viewer.py`,
  `tests\test_parity_diff_report.py`, `tests\test_generate_falcon_ecb_rust.py`,
  `tests\test_extract_melee_resources.py`, `tests\test_slippi_replay_tools.py`,
  `tests\test_launch_inputs.py`: `112 passed`.
- `tools\state_graph_viewer.py --check`: passed.
- `cargo run -p mole_cli -- parity --json`: `103` matches, `0` actionable
  value diffs.
- `cargo run -p mole_cli -- parity snapshot --json`: branch and remote checks
  passed; generated artifacts are present.
- `git diff --check`: no whitespace/conflict errors; Windows CRLF warnings only.

## Known Next Work

- Continue reducing remaining state-graph partials from the bottom up.
- Use human stick-roll captures and Slippi replay traces to compare native WUP
  input, HSD shaping, UCF amendments, and Rust state results.
- Keep replacing shortcut mechanics with source-shaped callback/data paths.
- Do not tune by feel first; use feel as a diagnostic after decomp/DAT
  comparison.
