# Complete bounded Optimal frame benchmark

The baseline contains 24 cases selected before projection: levels 11 and 12 at
64 and 2048 bytes for uniform, periodic and noise sources; minimum matches 3
through 7 at target lengths 1 and 4095; and the independently observed complete
wide-horizon and window-16 cases. It is projected only after complete offline
replay of the 1269-case bidirectional native capture. Both Rust backends must
produce identical full frames and both pinned normal and fully instrumented C
decoders must reproduce each complete source. Compressed bytes need not match C.

`tests/fixtures/reference-optimal-frame-baseline/cases.bin` is a bounded 221648-byte
little-endian `ZOBF0001` transport. Each row retains its request, independently
resolved tuple, complete source and complete compressed frame. The provenance
records the capture evidence digest, full totals, native revision, original
recipe manifest and per-row source/frame hashes. The immutable pack SHA-256 is
`de39c27b0eb6a48a17a07f7a23e7d949a1d40ec29de2be0e586b37337555bee9`.
The independent reader checks exact extents, row order, strict JSON types,
request/source identities against the frozen manifest and complete frame hashes.
Its normal and optimized offline controls reject 23 corrupted or forged records,
including structural checks with deliberately updated outer digests. Root digest
constants are trusted; this does not claim resistance to replacing the checker.

`examples/optimal_benchmark.rs` verifies complete pinned frames and decodes each
source outside timing. A separate allocator survey checks exact owner cleanup;
all 21 backend configurations must have identical frames and allocation counters.
Clock-free runs contain no timing samples. Timed runs measure Global one-shot
compression plus returned-output destruction with five samples of at least
100 ms per case. Input generation, file I/O, allocator instrumentation, decoding
and complete-frame comparisons remain outside the measured interval.

`scripts/benchmark_optimal_frames.py` builds every requested executable before
running any workload and pins source, compiler, Cargo configuration, environment,
arguments, logs, executables and frames. Its offline mode checks full evidence,
not just reported averages. Timing requires completed native, platform,
correctness-matrix and 21-backend clock-free evidence, plus a coordinator record
that no task-owned correctness jobs remain. This cannot prove an otherwise idle
desktop. Fixed-order results are diagnostic and must not establish a general
speedup. A separate safe/all repeat is required before milestone reporting.

## Clock-free qualification

All 21 configurations pass 24 cases each (504 complete frames) with identical
full frames and allocation counters. Both Python modes pass 43 offline checks
(two unchanged positives and 41 forgeries rejected), including a coherent
all-backend frame mutation rejected by the external baseline. No child process
is launched by these controls. Strict all-feature example Clippy and formatting
pass. The example is also included in the complete public safe regression.

The checked archives are `2026-10-16-optimal-frame-projection-v2`,
`2026-10-16-optimal-frame-baseline-reader`, `2026-10-16-optimal-frame-clock-free`,
`2026-10-16-optimal-frame-benchmark-controls`, and
`2026-10-16-optimal-frame-harness-checks`, under `docs/evidence/`. Each has a JSON
member inventory and SHA-256 hashes. These results contain no timing claim.

## Measured campaigns

The final full correctness matrix, native interoperability, platform checks and
clock-free campaign had all exited before the first timed campaign. All 21
executables were built before any timing began. The independent safe/all repeat
started only after the first campaign exited and its complete evidence passed
validation, with a fresh coordinator barrier. No task-owned correctness or archive
job overlapped either campaign. Desktop activity, clock and thermal conditions
were not controlled; backend order was fixed rather than randomized.

The first campaign contains 504 case/backend runs; the separate repeat contains
48. Each has five samples of at least 100 ms: 2760 retained samples in total.
Compression and output destruction use the Global allocator inside the interval;
loop and clock-check overhead are also included. All complete frame bytes and
separate allocation surveys match across clock-free, timed and repeat campaigns.
All 44 independently built executable files also match their corresponding
clock-free backend digest exactly. Selected allocation surveys range from 26 to
60 attempts and 185870 to 538922 peak bytes; these are workload observations,
not universal memory bounds.

| Campaign | All/safe geometric mean of median ns/byte | Smallest case ratio | Largest case ratio |
| --- | ---: | ---: | ---: |
| 21-backend campaign | 0.9493 | 0.8181 | 1.0204 |
| Independent safe/all repeat | 0.9393 | 0.6972 | 1.0298 |

A ratio below one means fewer measured nanoseconds per source byte. These two
aggregates favor all features, but individual cases are slower and the case
ratios move between campaigns. They do not establish a stable or general unsafe
speedup, statistical significance, or performance relative to upstream C.

The following first-campaign aggregates compare each backend with safe over the
same 24 complete-frame cases. Features unrelated to the active path are included
because they are independently supported build configurations; small differences
must not be attributed to a particular data structure without further evidence.

| Backend | Geometric mean relative ns/byte | Smallest case ratio | Largest case ratio |
| --- | ---: | ---: | ---: |
| `safe` | 1.0000 | 1.0000 | 1.0000 |
| `unsafe-byte-cursor` | 0.9830 | 0.8872 | 1.0957 |
| `unsafe-bitstream` | 0.9664 | 0.8936 | 1.1515 |
| `unsafe-history` | 0.9862 | 0.9038 | 1.1121 |
| `unsafe-ring-history` | 0.9864 | 0.8821 | 1.1261 |
| `unsafe-fse` | 1.0984 | 0.9012 | 3.1489 |
| `unsafe-fse-encoder` | 1.0407 | 0.8932 | 3.6536 |
| `unsafe-huffman` | 1.0208 | 0.8747 | 1.9410 |
| `unsafe-huffman-codebook` | 0.9555 | 0.8814 | 1.0454 |
| `unsafe-match-table` | 0.9441 | 0.8490 | 1.0233 |
| `unsafe-match-chain` | 0.9407 | 0.8478 | 1.0224 |
| `unsafe-dictionary` | 0.9509 | 0.8932 | 1.0079 |
| `unsafe-training` | 0.9534 | 0.8942 | 1.0055 |
| `unsafe-reference-chain` | 0.9532 | 0.8681 | 1.0843 |
| `unsafe-row-table` | 0.9545 | 0.8579 | 1.0548 |
| `unsafe-reference-tree` | 0.9405 | 0.8615 | 1.0098 |
| `unsafe-optimal-frequencies` | 0.9471 | 0.8916 | 1.0128 |
| `unsafe-optimal-paths` | 0.9465 | 0.8681 | 1.0516 |
| `unsafe-optimal-matches` | 0.9546 | 0.8542 | 1.0501 |
| `unsafe-optimal-short-key` | 0.9600 | 0.8907 | 1.0535 |
| `all` | 0.9493 | 0.8181 | 1.0204 |

[All per-case summaries](evidence/2026-10-16-optimal-frame-measurements.csv) retain
sizes, ratios, allocation counters, timing ranges and throughputs.
[The measurement manifest](evidence/2026-10-16-optimal-frame-measurements.json)
binds the three original campaigns, identical source maps and executable hashes.
Raw samples, complete stdout/stderr and frames, all 44 executable files, source
snapshots, both barriers and replay scripts are retained in
`evidence/2026-10-16-optimal-frame-timed.tar.gz`; the matching JSON inventories and
hashes every member. All archive members were verified after writing.

Both timed campaigns pass offline validation under normal and optimized Python.
The timing/barrier controls pass 26 checks per mode (two unchanged positives and
24 rejected forgeries), with zero attempted child processes. They cover sample
count, strict integer types, minimum duration, summary arithmetic, pending jobs,
missing or malformed prerequisite evidence, and coherently altered prerequisite
JSON with updated outer hashes. Their complete copied evidence is retained;
trusted root constants/checker replacement is outside their claim.
