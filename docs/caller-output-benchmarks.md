# Caller-owned one-shot output measurements

The frozen campaign measures all 22 configurations (safe, twenty isolated unsafe
features and all features), followed by independently rebuilt safe/all repeats.
Each case/mode has five samples. Every executable first passed identical output
and allocation qualification without timing. Correctness, native/platform,
corpus and final feature-matrix gates are hash-bound before measurement.

Caller destination allocation and deterministic source preparation occur before
clocks. Cold decode/session construction, compressed scratch allocation, frame
validation and destruction occur inside each timed iteration. Tracking allocators
are used only for separate clock-free qualification; timed calls use Global.
The owned comparison includes its result allocation and drop; direct output
reuses the preallocated caller destination. Each iteration decodes a complete
input, including its frame validation.

The 10 workloads are: `mixed-04`, `raw-07`, `rle-03`, `compressed-05`, `small-blocks-07`, `large-content-04`, `http`, `larger`, `empty`, `concatenated`.
The empty workload is excluded from geometric means and retained separately in
raw data. Per-case medians have equal weight; these are latency ratios, not a
byte-weighted throughput average. Ratios below one mean lower observed latency.

## All-backend median-latency comparisons

| Campaign | Backend | Schedule | direct / owned | direct / safe direct |
| --- | --- | --- | ---: | ---: |
| primary | `safe` | complete input | 0.5459 | 1.0000 |
| primary | `unsafe-bitstream` | complete input | 0.5360 | 0.9731 |
| primary | `unsafe-byte-cursor` | complete input | 0.5349 | 0.9676 |
| primary | `unsafe-dictionary` | complete input | 0.5445 | 0.9996 |
| primary | `unsafe-fse` | complete input | 0.5438 | 1.0079 |
| primary | `unsafe-fse-encoder` | complete input | 0.5419 | 0.9859 |
| primary | `unsafe-history` | complete input | 0.5559 | 0.9707 |
| primary | `unsafe-huffman` | complete input | 0.5430 | 0.9913 |
| primary | `unsafe-huffman-codebook` | complete input | 0.5414 | 0.9880 |
| primary | `unsafe-match-chain` | complete input | 0.5433 | 0.9773 |
| primary | `unsafe-match-table` | complete input | 0.5418 | 0.9980 |
| primary | `unsafe-optimal-frequencies` | complete input | 0.5399 | 0.9968 |
| primary | `unsafe-optimal-matches` | complete input | 0.5412 | 0.9753 |
| primary | `unsafe-optimal-paths` | complete input | 0.5471 | 1.0012 |
| primary | `unsafe-optimal-short-key` | complete input | 0.5456 | 0.9848 |
| primary | `unsafe-reference-chain` | complete input | 0.5409 | 0.9710 |
| primary | `unsafe-reference-tree` | complete input | 0.5334 | 1.0025 |
| primary | `unsafe-ring-history` | complete input | 0.5350 | 0.9866 |
| primary | `unsafe-row-table` | complete input | 0.5472 | 0.9907 |
| primary | `unsafe-slice-history` | complete input | 0.5390 | 0.9814 |
| primary | `unsafe-training` | complete input | 0.5326 | 1.0049 |
| primary | `all` | complete input | 0.5534 | 0.9698 |
| repeat | `safe` | complete input | 0.5483 | 1.0000 |
| repeat | `all` | complete input | 0.5568 | 0.9763 |

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
| `mixed-04` | direct | 16934 | 20 | 12309 | 4980 | 20 |
| `mixed-04` | owned | 16934 | 32 | 49164 | 22983 | 21 |
| `raw-07` | direct | 373 | 0 | 0 | 0 | 0 |
| `raw-07` | owned | 373 | 3 | 1274 | 728 | 1 |
| `rle-03` | direct | 2993 | 0 | 0 | 0 | 0 |
| `rle-03` | owned | 2993 | 5 | 7783 | 4116 | 1 |
| `compressed-05` | direct | 1337 | 6 | 1237 | 1221 | 6 |
| `compressed-05` | owned | 1337 | 14 | 4806 | 3013 | 7 |
| `small-blocks-07` | direct | 801 | 20 | 8037 | 6015 | 20 |
| `small-blocks-07` | owned | 801 | 28 | 10077 | 7039 | 21 |
| `large-content-04` | direct | 1010 | 4 | 3333 | 3333 | 4 |
| `large-content-04` | owned | 1010 | 7 | 6063 | 4888 | 5 |
| `http` | direct | 1189 | 7 | 1924 | 1541 | 7 |
| `http` | owned | 1189 | 13 | 4696 | 2949 | 8 |
| `larger` | direct | 143 | 0 | 0 | 0 | 0 |
| `larger` | owned | 143 | 1 | 143 | 143 | 1 |
| `empty` | direct | 0 | 0 | 0 | 0 | 0 |
| `empty` | owned | 0 | 0 | 0 | 0 | 0 |
| `concatenated` | direct | 18271 | 26 | 13546 | 4980 | 26 |
| `concatenated` | owned | 18271 | 38 | 50401 | 22983 | 27 |

The source, deterministic fixtures, all retained executables, build/qualification
logs, CPU barrier, all five raw samples per case/mode, median/minimum/maximum CSV
and complete per-case ratio JSON are retained in
`evidence/2026-10-26-caller-output-benchmarks.{json,tar.gz}`.
The primary has 2200 raw sample rows and the repeat
has 200; no unfinished sample is counted. The
recorded compiler/OS observations describe this host at preparation time, not
unrecorded environment values from earlier builds.
