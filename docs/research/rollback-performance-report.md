# Rollback Performance Report

Date: 2026-07-16

## Scope

This report measures the production `step_world_with_source_collisions` path and
whole-frame rollback resimulation at depths 1 through 8. It does not measure SDL
rendering, UDP latency, or input-device polling. Gameplay remains authoritative at
60 Hz; the 120/180/240 Hz host cadence only creates earlier opportunities to receive
packets and begin correction.

Command:

```text
cargo run --release -p mole_runtime --example rollback_benchmark -- 300
```

Reference machine:

- Windows 10 10.0.19045
- Intel Core i7-6700K, 4 cores / 8 logical processors, 4.00 GHz
- `rustc 1.95.0 (59807616e 2026-04-14)`, x86_64-pc-windows-msvc

The benchmark preloads immutable source data before timing. Each rollback sample
corrects frame 0 and resimulates the requested number of complete 60 Hz frames.

## Results

| Work | Depth | p50 ms | p95 ms | p99 ms | Max ms | Retained records | Retained inputs |
|---|---:|---:|---:|---:|---:|---:|---:|
| Ordinary simulation | 0 | 1.055 | 1.983 | 2.412 | 2.998 | 0 | 0 |
| Rollback | 1 | 0.776 | 1.195 | 2.235 | 3.737 | 1 | 1 |
| Rollback | 2 | 1.542 | 2.891 | 4.246 | 5.331 | 2 | 2 |
| Rollback | 3 | 2.274 | 3.675 | 6.478 | 10.229 | 3 | 3 |
| Rollback | 4 | 2.783 | 4.304 | 5.682 | 6.451 | 4 | 4 |
| Rollback | 5 | 4.280 | 6.493 | 8.915 | 11.251 | 5 | 5 |
| Rollback | 6 | 5.441 | 12.730 | 23.910 | 38.089 | 6 | 6 |
| Rollback | 7 | 5.695 | 13.014 | 23.566 | 49.447 | 7 | 7 |
| Rollback | 8 | 6.718 | 15.963 | 23.899 | 34.361 | 8 | 8 |

## Interpretation

The measured ordinary simulation p99 is below the 4.167 ms host-pass budget for
240 Hz on this machine. This is evidence for the current reference machine only;
it does not establish a low-end-hardware guarantee.

Seven-frame rollback, matching Slippi's maximum window, has a 5.695 ms median and
13.014 ms p95. The p95 fits inside one 16.667 ms authoritative frame interval, but
the p99 and maximum do not. The cadence resolver must therefore demote when actual
work misses the selected host deadline rather than assuming 240 Hz is sustainable.

Rollback snapshots use copy-on-write storage for the hit-victim log, so an ordinary
snapshot does not clone that vector. Runtime source-action lookup is indexed rather
than linearly scanned. The harness reports bounded retained record/input counts.
The workspace does not currently provide allocator instrumentation, so allocation
counts and retained byte totals are not claimed here.
