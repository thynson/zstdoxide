# Retained Optimal complete-frame baseline

This is a clock-free baseline checkpoint for Module 20. No retained Optimal
performance result or unsafe speedup is established here.

Twenty-six cases were selected before clocks from the independently audited
public native capture. They cover source boundaries 16385, 24576, 24577 and
32768; uniform, noise, periodic, copied and mixed histories; minimum matches
3 through 7; target requests 0, 1 and 4095; and selected table/search endpoints.
Zero request fields retain source-aware automatic resolution. In particular,
requesting target zero does not promise a resolved zero target.

The immutable `ZORB0001` pack contains 688138 source bytes and 132267 frame bytes.
Its complete 824126-byte representation has SHA-256
`7804afd8512933ec0d94f09a0ccdc7b801580e484b99e07bdba0124672750ce0`.
Each source/request/resolved tuple and safe/all frame is checked against the
qualified native capture. Both normal and sanitized native decodes reproduce
the complete source. Ordinary and optimized Python produce the same projection,
with process creation prohibited.

An isolated benchmark candidate adds one example to an otherwise unchanged
production snapshot. Safe/all release builds produce all 26 expected complete
frames and identical allocation ledgers. Every ledger has balanced owner
creation/drop, zero final live allocations and no tracking violations. The
clock-free output contains an empty sample array for every case and explicitly
records `timed=false`. Both strict example Clippy checks pass. The two actual
executables, source snapshots, output frames and logs are retained.

The first draft omitted a test-support file required by formatting's module
resolution and failed. Its original source and log are retained; the successor
copies the complete tests tree before qualification. Main source was frozen
for a concurrent full regression, so the example and pack remain in the
isolated snapshot and evidence archive at this checkpoint.

Evidence: `evidence/2026-10-20-optimal-retained-frame-baseline.tar.gz`, SHA-256
`b0060e60af3dc12c6471cf9b85e421bca5474d9a3372d3b00bda9c6898b512cd`.
All 424 regular files reconstruct exactly, totaling 73714451 logical bytes.
Unchanged inherited fixtures are represented by an explicit Git revision,
blob, size and SHA-256 map after byte-for-byte comparison; their original
snapshots remain preserved. Both smoke executables are included directly.

Remaining gates are an all-backend baseline/preflight, independently checked
controller and forgery controls, complete public regression reconciliation,
CPU barriers, timings for all storage backends and an independent safe/all
repeat. Clock-free command durations are operational metadata, not benchmark
samples. Module 20 remains open.


## Main integration checkpoint (2026-10-21)

The exact frozen Rust example and pack are now present in the main repository as
`examples/retained_optimal_benchmark.rs` and
`tests/fixtures/reference-retained-optimal-frame-baseline/`. The additive offline
checker binds source-only requests, resolved tuples, all source/frame SHA-256
identities and block kinds to the immutable capture. Normal and optimized Python
agree. Main formatting and strict safe/all example Clippy pass. Codec source is
unchanged. The promotion receipt and full scoped source snapshot are retained in
`evidence/2026-10-21-optimal-retained-benchmark-promotion.{json,tar.gz}`.

The original external-only baseline description above records its historical
state. All-backend controller qualification and timings remain separate gates;
this integration alone makes no performance claim.


## Complete clock-free qualification (2026-10-21)

The active v2 controller now qualifies all 21 storage configurations. All 546 case
rows and complete frames agree, allocation ownership balances, and every sample
array remains empty. Both ordinary/optimized corrected control suites pass 43/43.
The original ineffective search-log mutation and its 42/43 result are retained
separately. Exact source, binaries, frame/ledger evidence and the controller audit
are documented in `reference-retained-optimal-benchmark-controller-review.md`.
Timing still awaits the full regression and observed task-owned CPU barrier.
