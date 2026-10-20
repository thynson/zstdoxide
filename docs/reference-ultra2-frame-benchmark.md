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

## Timing remains pending

Only global-allocator compression and output destruction belong inside clocks.
Fixture loading, decoding, exact byte comparisons and allocation tracking stay
outside. All requested binaries must be built before measurement. The planned
primary campaign covers all 21 configurations and the independent repeat covers
safe/all, with five samples of at least 100 milliseconds per complete case.

Timed execution requires a recorded CPU barrier after full regression, native
interoperability, platform and clock-free prerequisites have completed, with no
remaining task-owned CPU jobs. Full public regression is still in progress, so
no Ultra2 timing campaign or performance comparison is claimed here. Fixed run
order, thermal state and unrelated desktop activity will remain limitations even
after that barrier. The completed clock-free survey establishes neither an
unsafe speedup nor throughput equivalence to the native implementation.
