# Complete Ultra frame measurements

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
upstream compatibility from these measurements. The completed campaigns below
follow this preselected measurement plan.

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

## Timed complete frames (2026-10-19; archived 2026-10-20)

The primary campaign measures all 21 configurations; an independent second
campaign measures safe and all again. Each of the 26 cases has five samples,
each lasting at least 100 milliseconds. All 598 case/configuration records and
2990 samples pass the complete evidence validator. Sources, resolved requests,
frames and allocator ledgers agree with the clock-free baseline and across the
repeat. Both campaigns preserve unchanged source inventories and 23 exact
executables; their 56 build/environment/run commands complete successfully.

The table compares each case's median compression-plus-output-drop nanoseconds
per input byte with the corresponding safe median in the same campaign. Ratios
below one are faster. The geometric mean weights each of the 26 preselected
cases equally; the range is the minimum and maximum per-case ratio, not a
confidence interval. Raw elapsed times and iteration counts are retained, and
`measurement-report/cases.csv` contains all per-case medians and ratios.

| Campaign | Configuration | Geometric mean cost / safe | Per-case range |
| --- | --- | ---: | ---: |
| primary | `safe` | 1.0000 | 1.0000–1.0000 |
| primary | `unsafe-byte-cursor` | 1.0128 | 0.9482–1.1200 |
| primary | `unsafe-bitstream` | 1.0064 | 0.9739–1.0613 |
| primary | `unsafe-history` | 1.3973 | 1.0029–1.9666 |
| primary | `unsafe-ring-history` | 2.0334 | 1.6839–2.5857 |
| primary | `unsafe-fse` | 2.0019 | 1.2842–4.5777 |
| primary | `unsafe-fse-encoder` | 1.5224 | 1.2310–1.9117 |
| primary | `unsafe-huffman` | 1.4153 | 1.2069–2.2109 |
| primary | `unsafe-huffman-codebook` | 1.3336 | 1.1663–1.5530 |
| primary | `unsafe-match-table` | 1.5748 | 1.2576–2.3707 |
| primary | `unsafe-match-chain` | 1.3020 | 1.1629–1.5401 |
| primary | `unsafe-dictionary` | 1.3490 | 1.0677–1.9735 |
| primary | `unsafe-training` | 1.0154 | 0.9529–1.1750 |
| primary | `unsafe-reference-chain` | 1.1072 | 0.9372–1.7234 |
| primary | `unsafe-row-table` | 1.0089 | 0.9277–1.1260 |
| primary | `unsafe-reference-tree` | 1.0148 | 0.9563–1.0966 |
| primary | `unsafe-optimal-frequencies` | 1.2753 | 1.0039–2.1724 |
| primary | `unsafe-optimal-paths` | 1.1506 | 0.9817–1.6187 |
| primary | `unsafe-optimal-matches` | 1.1177 | 1.0112–1.2542 |
| primary | `unsafe-optimal-short-key` | 1.0855 | 0.9532–1.2622 |
| primary | `all` | 1.0395 | 0.9839–1.1250 |
| repeat | `safe` | 1.0000 | 1.0000–1.0000 |
| repeat | `all` | 0.8782 | 0.6225–1.0172 |

The all/safe aggregate direction changes between campaigns (1.0395 versus
0.8782). These measurements do not establish a stable unsafe speedup. Individual
feature configurations also vary substantially, and some features do not exercise
their unsafe owner on this compression path. Do not interpret those differences
as an isolated causal effect of that feature. Fixed run order, thermal state and
uncontrolled unrelated desktop activity remain confounders. No comparison with
native C throughput, universal performance claim or complete-format claim follows.

Before each timed campaign a source-bound CPU barrier records completed platform,
full regression, native interoperability and clock-free prerequisites, with no
remaining task-owned jobs. It does not claim an otherwise idle desktop. Exact
prerequisite records, process snapshot, aggregation drivers and preserved failed
aggregation attempts are included. The platform inventory supplement identifies
two historical omissions via exact prequalification Git blobs rather than
pretending those files were in the original contemporaneous inventory.

Normal and optimized Python each pass 33 timing controls (two positive replays
and 31 rejected forgeries), with zero child execution attempts. They reject bad
sample types/counts/durations, forged summaries, missing or incomplete prerequisite
records, pending jobs and coherently altered production-source identities. These
controls complement the clock-free frame/owner controls; they use the independent
safe/all repeat and execute no codec or build subprocess. The report generator
independently recomputes aggregate and per-case statistics from validated rows.

Evidence: `evidence/2026-10-20-ultra-frames-timed.tar.gz`, SHA-256
`a2123ec502856149516e6068bf9c3b4f78b354300d6f9a96db095d3ffbaba339`.
The archive retains all 5546 bundle files (84329493 logical bytes), including
samples, frames, exact binaries, frozen sources, controls, prerequisite records
and CSV/JSON reports. Independent archive replay reconstructs every file and a
second read verifies every original file remained unchanged. The bundle records
736 individually hashed omitted Cargo intermediates; no measured executable or
log is omitted. Captured absolute paths describe original execution locations;
read the archive reconstruction instructions before replaying relocated evidence.
