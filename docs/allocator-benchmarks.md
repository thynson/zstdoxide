# Allocator-aware codec benchmarks

`benches/allocators.rs` compares eleven deterministic workloads under Global,
owned tracking, and borrowed non-Clone tracking allocation. The driver
`scripts/benchmark_allocators.py` verifies all 33 rows in safe, each of the 12
individual unsafe-feature builds, and the all-features build. Its default mode
performs correctness and allocation surveys without starting a Rust timing
clock. Pass `--timed` only after concurrent verification work has ended:

```sh
python3 scripts/benchmark_allocators.py /tmp/zstdoxide-allocator-smoke
python3 scripts/benchmark_allocators.py /tmp/zstdoxide-allocator-timed --timed
```

The source/configuration/fixture manifest is checked after every backend run;
compiler identity, commands, environment, and raw output are retained beside
the generated report. Timing defaults to five samples of at least 150 ms each.
`ZSTDOXIDE_ALLOCATOR_BENCH_SAMPLES` and `ZSTDOXIDE_ALLOCATOR_BENCH_MS` allow
bounded overrides, which are recorded in metadata. The driver validates sample
arithmetic and compares all non-timing fields across backend configurations.

Streaming operations measure reset/process with reused contexts and caller
buffers. One-shot operations, dictionary preparation, and training measure
fresh allocation and result destruction. Full-byte comparisons, fingerprints,
corpus/dictionary setup, and independent allocation surveys are outside timing.
The streaming drivers retain progress checks and black-box observation.

Tracking wraps Global and includes the test helper's locking, ledger and owner
identity costs. It is not another heap implementation. Global allocation counts
are uninstrumented `null` values. Each fresh survey records construction and
one cold operation, optional streaming reset, and final destruction; peaks are
cumulative high-water marks. Returned payload layouts exclude allocator internal
bookkeeping, rounding beyond returned sizes, and transient old/new Global resize
overlap. Caller buffers, fixture owners, and the tracking ledger are outside the
measured payload domain. Reachable retained capacity is a separate metric:
borrowed dictionary capacity is included even though its payload belongs to an
external owner. Dictionary/training result capacity is `null` because the public
API does not expose it.

## Harness checkpoint

All 14 clock-free backend runs passed, totaling 462 rows. Complete output bytes
matched each workload's Global reference; fingerprints, all allocation snapshots
and capacities matched across backend builds. Every tracked final-drop snapshot
had zero live payload, failures, and violations. Owned clones used distinct
identities; borrowed tracking kept one identity. Strict whole-crate Clippy,
owned-file formatting, Python compilation, and 12 parser positive/negative
checks passed. This checkpoint contains no throughput measurements.

The [harness evidence](evidence/2026-10-09-allocator-benchmark-harness.json)
records the exact checks and source hashes. The
[complete smoke archive](evidence/2026-10-09-allocator-benchmark-smoke.tar.gz)
contains all JSONL output, Cargo logs, source and environment manifests,
commands, parsed results and report. Its source fingerprint is
`2b2c51a395d53d658db1089fdbbc7937c28f5543759c5962a09729d808d11686`.
