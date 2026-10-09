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

## Timed checkpoint

All 14 configurations completed after the codec/platform verification processes
ended. Each of 33 workload/allocation-mode rows has five samples of at least
150 ms, totaling 462 rows and 2,310 raw samples. The source fingerprint remains
`2b2c51a395d53d658db1089fdbbc7937c28f5543759c5962a09729d808d11686`.
The compiler is the exact Module 9 nightly pin on aarch64-apple-darwin.

Configurations ran in fixed order: safe, the twelve individual unsafe features,
then all features. No concurrent codec test, compiler or Miri campaign was
scheduled. Light documentation/evidence work continued; OS scheduling, thermal
state, frequency changes and code layout can still affect measurements. These
five-sample observations are not randomized confidence intervals or proof of a
universal speedup. Tracking measurements include their Global-backed helper
overhead, not a different heap implementation.

The table compares Global median ns per workload byte, followed by the sample
range. Codec operations use uncompressed bytes, dictionary preparation uses
serialized dictionary bytes, and training uses sample bytes. The ratio is
all-features divided by safe, so values below one indicate lower observed time.

| Workload | Safe median [min, max] | All features median [min, max] | All / safe |
| --- | ---: | ---: | ---: |
| one_shot_encode | 12.055 [11.859, 12.069] | 9.724 [9.712, 10.063] | 0.807 |
| one_shot_decode | 1.386 [1.378, 1.396] | 1.120 [1.092, 1.197] | 0.808 |
| stream_encode_greedy | 9.418 [9.196, 13.093] | 9.131 [9.104, 9.234] | 0.970 |
| stream_encode_chain | 10.920 [10.327, 11.426] | 10.819 [10.555, 10.881] | 0.991 |
| stream_encode_lazy | 14.346 [13.177, 14.740] | 13.841 [13.412, 14.109] | 0.965 |
| stream_decode | 2.975 [2.856, 2.991] | 2.791 [2.781, 2.881] | 0.938 |
| prepare_dictionary | 3.568 [3.563, 3.580] | 3.633 [3.530, 3.680] | 1.018 |
| dictionary_stream_encode | 10.225 [10.207, 10.277] | 10.204 [9.995, 10.226] | 0.998 |
| dictionary_stream_decode | 5.866 [5.684, 7.058] | 5.707 [5.566, 6.494] | 0.973 |
| raw_training | 8.670 [8.570, 8.690] | 8.562 [8.460, 8.592] | 0.988 |
| formatted_training | 10.127 [9.841, 10.200] | 9.687 [9.671, 10.407] | 0.957 |

All non-timing output/metric/capacity rows matched across backend configurations.
Both tracking modes used the same allocation request counts and returned-layout
payload sizes for these inputs; owned clones retained distinct allocator
identities, while borrowed tracking retained one. Global counts remain
uninstrumented. Selected cold-operation accounting makes the resource cost
visible independently of timing:

| Workload | Tracked request attempts | Tracked peak payload bytes | Reachable retained capacity after reset |
| --- | ---: | ---: | ---: |
| one_shot_encode | 132 | 1100879 | not a streaming context |
| one_shot_decode | 15 | 41210 | not a streaming context |
| stream_encode_greedy | 134 | 1330242 | 1263821 |
| stream_encode_chain | 135 | 10192861 | 10056019 |
| stream_encode_lazy | 135 | 10192530 | 10055656 |
| stream_decode | 7 | 75047 | 68653 |
| dictionary_stream_encode | 140 | 2052388 | 1939204 |
| dictionary_stream_decode | 7 | 3713 | 17099 |
| raw_training | 20 | 299008 | not a streaming context |
| formatted_training | 48 | 299008 | not a streaming context |

The dictionary stream rows include externally borrowed dictionary capacity in
the reachable metric, while their tracked payload excludes that external owner.
Reset retained the measured streaming capacities; it did not implement shrinking.
Every tracked final-drop state reported zero live payload and no violations or
failures. Complete Global/owned/borrowed timings, all individual-feature results,
all samples, allocation survey phases, source/environment manifests and logs
are retained in the [timed evidence archive](evidence/2026-10-09-allocator-benchmark-timed.tar.gz).
The [consolidated record](evidence/2026-10-09-allocator-benchmark-timed.json)
identifies exact commands and their verified artifacts.
