# Complete Ultra2 frame qualification and measurement plan

The frozen corpus contains 26 complete sources: uniform, periodic and noise
inputs at 64 and 2048 bytes for default levels 16 and 17; minimum settings 3..7
crossed with targets 1/4095 at 16384 bytes; and the wide-horizon, window16 and two
alternative-branch sources. Selection was recorded before the new native capture.
No observed timing selects or removes a workload. These artificial cases are not
a general compression-ratio corpus.

Every baseline frame passed independent native capture replay, safe/all Rust
equality and normal/sanitized C decoding. The immutable pack owns exact source,
request and complete frame bytes. Borrowed baseline views and owned measurement
records are plain values and share their representation across backends.
The pack SHA-256 is
`5800d0e13d324f2401f2739e1482ad8d9eb8afe42f2118933afa42b7b1b3896f`.
Its selection, provenance and independent validation are retained in
`evidence/2026-10-20-ultra2-frame-baseline.tar.gz` (SHA-256
`9460dc99f255934d5f3001102432cfbc73321b4ba44aeb1d38286f334d070de5`).

## Completed clock-free survey

All 21 configurations pass the 26-case survey: 546 complete case executions.
The configurations are the safe default, each of the nineteen isolated unsafe
features and all features together. Every saved complete frame equals the
independently native-audited baseline. Resolved requests, source identities and
complete allocation ledgers agree across configurations. No elapsed samples are
present. The campaign records 47 successful commands, 98 source identities and
all 21 actual compiled executables.

Complete frames range from 16 to 4140 bytes. Tracked peak requested memory ranges
from 194062 to 538922 bytes; allocator identities created and dropped range from
28 to 97. Every execution ends with zero live blocks and bytes, without allocator
violations. These whole-frame counters include entropy and output workspaces;
they are distinct from the private two-pass coordinator's nine preserved owners.
Backend equality and internal ledger consistency do not constitute an
independently predicted native allocation count.

Normal and optimized Python each pass 43 copied-evidence controls: two accepted
positive replays and 41 rejected forgeries, with zero child-process attempts.
Controls cover exact source and command identities, environments, complete frame
bytes, coherent all-backend frame forgery, allocator counters, resolved parameter
types and unexpected clocks. The earlier isolated safe/all pre-admission survey
is retained separately from the complete main-checkout campaign.

Clock-free evidence: `evidence/2026-10-20-ultra2-clockfree.tar.gz`, SHA-256
`e728045d2e8a36f56ca5dd198af71a4531b11acb2b239852effb1831bd010cfc`.
The lossless archive reconstructs all 56,058 regular bundle files (124,643,519
logical bytes), including source snapshots, complete frames, all 21 executables,
control artifacts and the earlier smoke evidence. Independent reconstruction
and a second unchanged-original hash check pass. The bundle individually hashes
672 omitted redundant Cargo intermediates; no campaign executable or log is
omitted. Captured absolute paths describe original execution locations and are
not rewritten to imply a relocated rerun.

## Timed complete frames (2026-10-20)

The primary campaign measures all 21 configurations; an independent second
campaign measures safe and all again. All 26 preselected cases have five samples
of at least 100 milliseconds. The complete validator accepts 598 case/backend
records and 2990 samples. Sources, resolved requests, complete frame bytes and
allocation ledgers agree with the clock-free survey and across the repeat.
The campaigns retain all 23 measured executables and 56 successful commands.

Only global-allocator compression and output destruction belong inside clocks.
Fixture loading, native-baseline comparison, decoding and allocation tracking
stay outside. All requested executables were built before measuring. Recorded
CPU barriers follow completed regression, platform, native and clock-free gates;
no other task-owned CPU job ran during either campaign. The independent repeat
started after every primary child command exited.

The table compares each case's median compression-plus-drop nanoseconds per
input byte with the corresponding safe median in the same campaign. A ratio
below one is faster. Geometric means weight the 26 cases equally. The range
contains minimum and maximum per-case ratios, not a confidence interval.
Raw elapsed times and iteration counts are retained; the archived
`measurement-report/cases.csv` contains all individual results.

| Campaign | Configuration | Geometric mean cost / safe | Per-case range |
| --- | --- | ---: | ---: |
| primary | `safe` | 1.0000 | 1.0000–1.0000 |
| primary | `unsafe-byte-cursor` | 0.9940 | 0.9580–1.0372 |
| primary | `unsafe-bitstream` | 0.9966 | 0.9527–1.0874 |
| primary | `unsafe-history` | 0.9957 | 0.9628–1.0289 |
| primary | `unsafe-ring-history` | 0.9937 | 0.9536–1.0333 |
| primary | `unsafe-fse` | 1.0565 | 0.9819–1.5034 |
| primary | `unsafe-fse-encoder` | 0.9957 | 0.9576–1.0428 |
| primary | `unsafe-huffman` | 0.9869 | 0.9626–1.0095 |
| primary | `unsafe-huffman-codebook` | 0.9971 | 0.9616–1.0408 |
| primary | `unsafe-match-table` | 1.0027 | 0.9588–1.0896 |
| primary | `unsafe-match-chain` | 1.0390 | 0.9652–1.9881 |
| primary | `unsafe-dictionary` | 0.9958 | 0.9707–1.0324 |
| primary | `unsafe-training` | 1.0073 | 0.9605–1.1806 |
| primary | `unsafe-reference-chain` | 0.9943 | 0.9472–1.0340 |
| primary | `unsafe-row-table` | 0.9876 | 0.9535–1.0293 |
| primary | `unsafe-reference-tree` | 0.9976 | 0.9694–1.0355 |
| primary | `unsafe-optimal-frequencies` | 0.9953 | 0.9507–1.0232 |
| primary | `unsafe-optimal-paths` | 1.0270 | 0.9486–1.6823 |
| primary | `unsafe-optimal-matches` | 1.0225 | 0.9585–1.1840 |
| primary | `unsafe-optimal-short-key` | 0.9871 | 0.9569–1.0107 |
| primary | `all` | 0.9985 | 0.9634–1.1163 |
| repeat | `safe` | 1.0000 | 1.0000–1.0000 |
| repeat | `all` | 1.0007 | 0.9733–1.0216 |

The all/safe geometric means are 0.9985 and 1.0007. Their near-unity values and
opposing directions do not establish a stable unsafe speedup. Individual features
have some larger outliers, and not every unsafe owner is exercised by this
compression path. Fixed run order, thermal state and unrelated desktop activity
prevent interpreting those differences as isolated causal effects of a feature.
No native-C throughput comparison or general compression-ratio claim follows.

Normal and optimized Python each pass 33 independent copied-evidence controls:
two accepted replays, 31 rejected timing/barrier/source forgeries and zero child
attempts. The independent summary revalidates both complete campaigns, source
identity and repeated frame/ledger equality before computing any aggregate.

Timed evidence: `evidence/2026-10-20-ultra2-frames-timed.tar.gz`, SHA-256
`5af25354f2a903127afc9c6ed477e6e4a5f8c0fdc70cffe9d806e364860bd46e`.
The lossless archive preserves all 5544 regular bundle files (100520358 logical
bytes), including samples, source snapshots, frames, all 23 binaries, process
barriers, prerequisites, report tools and control artifacts. Full reconstruction
and a second unchanged-original hash check pass. The bundle separately hashes
736 omitted redundant Cargo intermediates; no measured executable or log is
omitted. Captured paths retain their actual original execution locations.
