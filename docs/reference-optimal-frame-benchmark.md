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
