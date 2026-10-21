# Retained Optimal complete-frame measurements

Module 20 now has complete all-backend measurements and an independent safe/all
repeat. The historical clock-free checkpoints below precede the measured results
at the end of this document. All ratios are compression-plus-drop cost per byte,
so less than one is faster.

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

At the original baseline checkpoint, remaining gates were an all-backend
baseline/preflight, independently checked
controller and forgery controls, complete public regression reconciliation,
CPU barriers, timings for all storage backends and an independent safe/all
repeat. Clock-free command durations are operational metadata, not benchmark
samples. These historical pending gates are completed below.


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
state. Its integration alone made no performance claim; subsequent controller
and measurement qualification are recorded below.


## Complete clock-free qualification (2026-10-21)

The active v2 controller now qualifies all 21 storage configurations. All 546 case
rows and complete frames agree, allocation ownership balances, and every sample
array remains empty. Both ordinary/optimized corrected control suites pass 43/43.
The original ineffective search-log mutation and its 42/43 result are retained
separately. Exact source, binaries, frame/ledger evidence and the controller audit
are documented in `reference-retained-optimal-benchmark-controller-review.md`.
That clock-free checkpoint preceded the full regression and task-owned CPU
barrier used for the completed measurements below.


## Complete measurements and independent repeat (2026-10-21)

The completed public correctness, foreign-platform, native-source reconciliation
and clock-free receipts are bound by exact hashes before timing. A retained
process snapshot confirms no task-owned build, test, interpreter, control or
archive job remained. All 21 primary executables are built before measurement;
the independently built safe/all repeat starts only after the primary campaign
and all its children complete. No competing task-owned CPU work runs during
either campaign. This barrier does not establish unrelated desktop idleness.

Each of 26 preselected cases has five samples with a minimum 100 ms measurement
budget per sample. The primary campaign retains 546 case rows; the repeat adds
52, for 598 rows and 2990 samples across 23 actual executables. Complete frames,
resolved requests and allocation ledgers agree across all backends and across
the repeat. Owner creation/drop balances and final live allocation is zero.
Ordinary and optimized timed-controller suites each pass 33 checks: 31 rejected
forgeries and two positives, with no child execution.

Ratios below use the median compression-plus-drop nanoseconds per source byte
for each case, divided by its same-campaign safe median. Geometric means weight
all 26 cases equally. Minimum/maximum columns are the range of per-case ratios,
not confidence bounds. Raw five-sample ranges and every executable identity are
retained in the report CSV and source-bound evidence.

| Campaign | Configuration | Geometric mean cost / safe | Per-case minimum | Per-case maximum |
| --- | --- | ---: | ---: | ---: |
| primary | `safe` | 1.0000 | 1.0000 | 1.0000 |
| primary | `unsafe-byte-cursor` | 0.9925 | 0.9527 | 1.0059 |
| primary | `unsafe-bitstream` | 0.9991 | 0.9713 | 1.0201 |
| primary | `unsafe-history` | 0.9996 | 0.9645 | 1.0386 |
| primary | `unsafe-ring-history` | 1.0339 | 1.0019 | 1.0581 |
| primary | `unsafe-fse` | 1.0009 | 0.9638 | 1.0336 |
| primary | `unsafe-fse-encoder` | 0.9956 | 0.9591 | 1.0211 |
| primary | `unsafe-huffman` | 0.9959 | 0.9589 | 1.0258 |
| primary | `unsafe-huffman-codebook` | 0.9981 | 0.9600 | 1.0160 |
| primary | `unsafe-match-table` | 0.9995 | 0.9603 | 1.0475 |
| primary | `unsafe-match-chain` | 0.9992 | 0.9585 | 1.0224 |
| primary | `unsafe-dictionary` | 1.0025 | 0.9738 | 1.0406 |
| primary | `unsafe-training` | 1.0058 | 0.9639 | 1.0596 |
| primary | `unsafe-reference-chain` | 1.0024 | 0.9591 | 1.0314 |
| primary | `unsafe-row-table` | 0.9975 | 0.9567 | 1.0287 |
| primary | `unsafe-reference-tree` | 0.9741 | 0.9068 | 1.0058 |
| primary | `unsafe-optimal-frequencies` | 1.0014 | 0.9606 | 1.0257 |
| primary | `unsafe-optimal-paths` | 0.9981 | 0.9609 | 1.0171 |
| primary | `unsafe-optimal-matches` | 1.0403 | 0.9770 | 1.1866 |
| primary | `unsafe-optimal-short-key` | 0.9908 | 0.9053 | 1.0547 |
| primary | `all` | 1.0228 | 0.9423 | 1.0696 |
| repeat | `safe` | 1.0000 | 1.0000 | 1.0000 |
| repeat | `all` | 1.0258 | 0.9331 | 1.1654 |

All-unsafe costs 1.0228 times safe in the primary campaign and 1.0258 in the
independent repeat: approximately 2.3% and 2.6% slower in these equally weighted
workloads. No universal unsafe speedup is established. Per-case behavior varies;
fixed execution order, compilation/thermal history, timer variability and
unrelated desktop work limit broader conclusions. Some isolated features do not
participate in this path, so small ratios near one do not demonstrate a benefit
from that owner. Safe remains the default implementation.

All executables, source inventories, CPU barriers, raw outputs/samples, controls,
CSV/JSON reports, corrected clock-free reconciliation and drivers are retained in
`evidence/2026-10-21-optimal-retained-frames-timed.{json,tar.gz}`. Archive SHA-256:
`a6774d16f5ae3a8cb391feff3594065f087d05e7ab936e09c7b80f4db5cb4680`.
The archive is reconstructed independently and its original files rehashed.
