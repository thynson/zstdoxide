# Complete Ultra frame measurement plan

The preselected corpus contains 26 complete sources: uniform/periodic/noise at
64 and 2048 bytes for default levels 13 and 14; minimum settings 3..7 crossed
with targets 1/4095 at 16384 bytes; and the ordinary horizon, window16 and two
alternative-branch sources. Names are frozen in `pack_ultra_frame_baseline.py`
before native capture. No observed timing selects or removes workloads.

Each baseline frame must first pass complete independent native capture replay,
safe/all Rust equality and normal/sanitized C decoding. The baseline pack owns
source/request/frame bytes; borrowed case views or owned measurement records
preserve exact complete byte identities. These plain records share representation
across backends. The clock-free survey compares exact source/frame fingerprints,
resolved requests and complete allocator ledgers before timing is permitted.

Only global-allocator compression and output destruction belong inside clocks.
Fixture loading, decoding, byte comparisons and allocation tracking stay outside.
All requested binaries are built before measurements. Run all nineteen isolated
unsafe features, safe and all together on identical deterministic requests, then
an independent safe/all repeat. Wait for builds, tests, native captures and audits
to finish and record a CPU barrier before timed runs. Report both directions when
aggregate performance disagrees, and do not infer C byte identity or complete
upstream compatibility from these measurements. No timings have been taken yet.

## Qualified clock-free survey

All 21 configurations pass the 26-case clock-free survey (546 complete case
executions). Every saved frame equals the independently native-audited baseline
and every allocator ledger equals the safe build's corresponding ledger.
The frozen pack SHA-256 is
`dce7dd5dc40283759314baf18daad00a385f739a157b02130b4d4cdaa0b33c75`.
No elapsed samples are present in these records. The campaign preserves 47
successful commands, 93 source identities and each exact compiled executable.

Across these fixed cases, complete frames range from 16 to 4140 bytes, tracked
peak requested memory ranges from 194062 to 538922 bytes, and total allocator
identities created and dropped range from 28 to 97. Those are whole-frame
measurements, including existing entropy/output workspaces; the private Ultra
parser's nine owners are a different, narrower contract. Every whole-frame run
ends with zero live blocks/bytes and no allocator violations. No allocation
count here is presented as an independently predicted native owner count.

Normal and optimized Python each pass 43 copied-evidence controls: two positive
replays and 41 rejected forgeries, with zero child attempts. Controls cover
command/environment/source identities, exact complete frame bytes, coherent
all-backend frame forgery, allocator counters, resolved parameter types and
unexpected clocks. The read-only baseline checker also passes both modes.

Clock-free evidence: `evidence/2026-10-19-ultra-clockfree.tar.gz`, SHA-256
`94f11819fc08b8c789e2acc9b66dc7ee5ed6e3ba42f19ac3ad52911099f40ae9`.
The lossless chunk archive reconstructs all 55,890 bundle files (119,775,177
logical bytes), including every compiled executable and all control artifacts.
The serial archive attempt was interrupted after diagnosing regular-file open
latency; its exact source and diagnostic are retained. The concurrent successor
independently reconstructed every archived file and rehashed every unchanged
original. The bundle separately inventories 672 omitted redundant Cargo build
intermediates; these omissions do not include any measured executable or log.
