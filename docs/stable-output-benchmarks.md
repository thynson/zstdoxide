# Stable borrowed incremental output measurements

The frozen campaign measures all 22 configurations (safe, twenty isolated unsafe
features and all features), followed by independently rebuilt safe/all repeats.
Each case/mode has five samples. Every executable first passed identical output
and allocation qualification without timing. Correctness, native/platform,
corpus and final feature-matrix gates are hash-bound before measurement.

Caller destination allocation and deterministic source preparation occur before
clocks. Cold decode/session construction, compressed scratch allocation, frame
validation and destruction occur inside each timed iteration. Tracking allocators
are used only for separate clock-free qualification; timed calls use Global.
Both stable and buffered destinations are allocated before timing. The buffered
path still allocates its internal history/block storage. Whole-input and
maximum-seven-byte call schedules are measured separately. Sessions are created
and dropped for every iteration; retained-session reuse is not measured.

The 10 workloads are: `mixed-04`, `raw-07`, `rle-03`, `compressed-05`, `small-blocks-07`, `large-content-04`, `http`, `larger`, `empty`, `concatenated`.
The empty workload is excluded from geometric means and retained separately in
raw data. Per-case medians have equal weight; these are latency ratios, not a
byte-weighted throughput average. Ratios below one mean lower observed latency.

## All-backend median-latency comparisons

| Campaign | Backend | Schedule | stable / buffered | stable / safe stable |
| --- | --- | --- | ---: | ---: |
| primary | `safe` | whole | 0.4722 | 1.0000 |
| primary | `safe` | fragmented | 0.5425 | 1.0000 |
| primary | `unsafe-bitstream` | whole | 0.4779 | 1.0301 |
| primary | `unsafe-bitstream` | fragmented | 0.5352 | 1.0227 |
| primary | `unsafe-byte-cursor` | whole | 0.4760 | 0.9985 |
| primary | `unsafe-byte-cursor` | fragmented | 0.5458 | 1.0176 |
| primary | `unsafe-dictionary` | whole | 0.4643 | 0.9971 |
| primary | `unsafe-dictionary` | fragmented | 0.5399 | 1.0144 |
| primary | `unsafe-fse` | whole | 0.4766 | 1.0189 |
| primary | `unsafe-fse` | fragmented | 0.5458 | 1.0297 |
| primary | `unsafe-fse-encoder` | whole | 0.4463 | 1.0025 |
| primary | `unsafe-fse-encoder` | fragmented | 0.5496 | 1.0380 |
| primary | `unsafe-history` | whole | 0.4598 | 0.9950 |
| primary | `unsafe-history` | fragmented | 0.5408 | 1.0109 |
| primary | `unsafe-huffman` | whole | 0.4711 | 1.0101 |
| primary | `unsafe-huffman` | fragmented | 0.5465 | 1.0241 |
| primary | `unsafe-huffman-codebook` | whole | 0.4740 | 1.0053 |
| primary | `unsafe-huffman-codebook` | fragmented | 0.5420 | 1.0235 |
| primary | `unsafe-match-chain` | whole | 0.4662 | 1.0268 |
| primary | `unsafe-match-chain` | fragmented | 0.5414 | 1.0055 |
| primary | `unsafe-match-table` | whole | 0.4684 | 1.0041 |
| primary | `unsafe-match-table` | fragmented | 0.5448 | 1.0192 |
| primary | `unsafe-optimal-frequencies` | whole | 0.4704 | 1.0030 |
| primary | `unsafe-optimal-frequencies` | fragmented | 0.5513 | 1.0250 |
| primary | `unsafe-optimal-matches` | whole | 0.4631 | 0.9818 |
| primary | `unsafe-optimal-matches` | fragmented | 0.5411 | 1.0099 |
| primary | `unsafe-optimal-paths` | whole | 0.4712 | 0.9868 |
| primary | `unsafe-optimal-paths` | fragmented | 0.5420 | 1.0036 |
| primary | `unsafe-optimal-short-key` | whole | 0.4713 | 1.0111 |
| primary | `unsafe-optimal-short-key` | fragmented | 0.5457 | 1.0321 |
| primary | `unsafe-reference-chain` | whole | 0.4717 | 1.0210 |
| primary | `unsafe-reference-chain` | fragmented | 0.5506 | 1.0306 |
| primary | `unsafe-reference-tree` | whole | 0.4739 | 1.0189 |
| primary | `unsafe-reference-tree` | fragmented | 0.5454 | 1.0306 |
| primary | `unsafe-ring-history` | whole | 0.4931 | 1.0228 |
| primary | `unsafe-ring-history` | fragmented | 0.5886 | 1.0370 |
| primary | `unsafe-row-table` | whole | 0.4692 | 1.0086 |
| primary | `unsafe-row-table` | fragmented | 0.5450 | 1.0274 |
| primary | `unsafe-slice-history` | whole | 0.4737 | 1.0167 |
| primary | `unsafe-slice-history` | fragmented | 0.5448 | 1.0237 |
| primary | `unsafe-training` | whole | 0.4716 | 1.0184 |
| primary | `unsafe-training` | fragmented | 0.5179 | 1.1429 |
| primary | `all` | whole | 0.4559 | 1.0298 |
| primary | `all` | fragmented | 0.6026 | 1.0916 |
| repeat | `safe` | whole | 0.5039 | 1.0000 |
| repeat | `safe` | fragmented | 0.5301 | 1.0000 |
| repeat | `all` | whole | 0.4782 | 0.9215 |
| repeat | `all` | fragmented | 0.5738 | 0.9729 |

These fixed-order host measurements do not establish statistical significance,
a general unsafe speedup or architecture-independent behavior. Some isolated
features do not participate in decoding; small changes for those configurations
are measurement variation, not evidence of a benefit from that implementation.
Thermal/compiler history, timer precision and unrelated desktop work can affect
results. The task-owned CPU barrier records that this task's build/test jobs had
finished; it does not claim the whole desktop was idle. Safe remains the default.

## Clock-free allocator observations

The following request and peak byte counts are identical across every primary
and independently rebuilt repeat configuration. All tracked payloads are released,
owner identities balance and output fingerprints agree. These are payload allocations charged to the selected codec allocator; owned
modes include their result allocation, while buffered modes
include internal history/block storage. Caller output and separately prepared
dictionary payloads are excluded. Tracking bookkeeping and shared-control
storage are outside these payload counts.

| Case | Mode | Output bytes | Allocation attempts | Requested bytes | Peak live bytes | Deallocations |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `mixed-04` | stable-whole | 16934 | 20 | 12309 | 4980 | 20 |
| `mixed-04` | buffered-whole | 16934 | 23 | 48761 | 41432 | 23 |
| `mixed-04` | stable-fragmented | 16934 | 21 | 14893 | 7564 | 21 |
| `mixed-04` | buffered-fragmented | 16934 | 23 | 48761 | 41432 | 23 |
| `raw-07` | stable-whole | 373 | 0 | 0 | 0 | 0 |
| `raw-07` | buffered-whole | 373 | 1 | 373 | 373 | 1 |
| `raw-07` | stable-fragmented | 373 | 0 | 0 | 0 | 0 |
| `raw-07` | buffered-fragmented | 373 | 1 | 373 | 373 | 1 |
| `rle-03` | stable-whole | 2993 | 0 | 0 | 0 | 0 |
| `rle-03` | buffered-whole | 2993 | 1 | 1664 | 1664 | 1 |
| `rle-03` | stable-fragmented | 2993 | 0 | 0 | 0 | 0 |
| `rle-03` | buffered-fragmented | 2993 | 1 | 1664 | 1664 | 1 |
| `compressed-05` | stable-whole | 1337 | 6 | 1237 | 1221 | 6 |
| `compressed-05` | buffered-whole | 1337 | 9 | 4358 | 4342 | 9 |
| `compressed-05` | stable-fragmented | 1337 | 7 | 1684 | 1668 | 7 |
| `compressed-05` | buffered-fragmented | 1337 | 9 | 4358 | 4342 | 9 |
| `small-blocks-07` | stable-whole | 801 | 20 | 8037 | 6015 | 20 |
| `small-blocks-07` | buffered-whole | 801 | 23 | 9974 | 7952 | 23 |
| `small-blocks-07` | stable-fragmented | 801 | 21 | 8372 | 6350 | 21 |
| `small-blocks-07` | buffered-fragmented | 801 | 23 | 9974 | 7952 | 23 |
| `large-content-04` | stable-whole | 1010 | 4 | 3333 | 3333 | 4 |
| `large-content-04` | buffered-whole | 1010 | 7 | 4983 | 4983 | 7 |
| `large-content-04` | stable-fragmented | 1010 | 5 | 3353 | 3353 | 5 |
| `large-content-04` | buffered-fragmented | 1010 | 7 | 4983 | 4983 | 7 |
| `http` | stable-whole | 1189 | 7 | 1924 | 1541 | 7 |
| `http` | buffered-whole | 1189 | 10 | 4735 | 4352 | 10 |
| `http` | stable-fragmented | 1189 | 8 | 2357 | 1974 | 8 |
| `http` | buffered-fragmented | 1189 | 10 | 4735 | 4352 | 10 |
| `larger` | stable-whole | 143 | 0 | 0 | 0 | 0 |
| `larger` | buffered-whole | 143 | 1 | 143 | 143 | 1 |
| `larger` | stable-fragmented | 143 | 0 | 0 | 0 | 0 |
| `larger` | buffered-fragmented | 143 | 1 | 143 | 143 | 1 |
| `empty` | stable-whole | 0 | 0 | 0 | 0 | 0 |
| `empty` | buffered-whole | 0 | 0 | 0 | 0 | 0 |
| `empty` | stable-fragmented | 0 | 0 | 0 | 0 | 0 |
| `empty` | buffered-fragmented | 0 | 0 | 0 | 0 | 0 |
| `concatenated` | stable-whole | 18271 | 26 | 13546 | 4980 | 26 |
| `concatenated` | buffered-whole | 18271 | 29 | 52672 | 44106 | 29 |
| `concatenated` | stable-fragmented | 18271 | 27 | 16130 | 7564 | 27 |
| `concatenated` | buffered-fragmented | 18271 | 29 | 52672 | 44106 | 29 |

The source, deterministic fixtures, all retained executables, build/qualification
logs, CPU barrier, all five raw samples per case/mode, median/minimum/maximum CSV
and complete per-case ratio JSON are retained in
`evidence/2026-10-26-stable-output-benchmarks.{json,tar.gz}`.
The primary has 4400 raw sample rows and the repeat
has 400; no unfinished sample is counted. The
recorded compiler/OS observations describe this host at preparation time, not
unrecorded environment values from earlier builds.
