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
