# Persistent dictionaries in stable borrowed output measurements

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
Dictionary preparation is outside clocks. Each new session clones and drops one selected dictionary handle inside timing.

The 11 workloads are: `raw-small-window`, `p0-far-dictionary`, `p-window-equal`, `dictionary-to-frame-eviction`, `formatted-id`, `dictionary-treeless-four-streams`, `dictionary-repeat-sequences`, `duplicate-reps-initial`, `persistent-concatenation`, `http`, `empty`.
The empty workload is excluded from geometric means and retained separately in
raw data. Per-case medians have equal weight; these are latency ratios, not a
byte-weighted throughput average. Ratios below one mean lower observed latency.

## All-backend median-latency comparisons

| Campaign | Backend | Schedule | stable / buffered | stable / safe stable |
| --- | --- | --- | ---: | ---: |
| primary | `safe` | whole | 0.4326 | 1.0000 |
| primary | `safe` | fragmented | 0.5493 | 1.0000 |
| primary | `unsafe-bitstream` | whole | 0.4509 | 1.0023 |
| primary | `unsafe-bitstream` | fragmented | 0.5722 | 0.9872 |
| primary | `unsafe-byte-cursor` | whole | 0.4474 | 0.9785 |
| primary | `unsafe-byte-cursor` | fragmented | 0.5686 | 0.9900 |
| primary | `unsafe-dictionary` | whole | 0.4493 | 1.0002 |
| primary | `unsafe-dictionary` | fragmented | 0.5664 | 0.9943 |
| primary | `unsafe-fse` | whole | 0.4505 | 0.9822 |
| primary | `unsafe-fse` | fragmented | 0.5615 | 0.9640 |
| primary | `unsafe-fse-encoder` | whole | 0.4482 | 1.0106 |
| primary | `unsafe-fse-encoder` | fragmented | 0.5747 | 1.0064 |
| primary | `unsafe-history` | whole | 0.4496 | 0.9951 |
| primary | `unsafe-history` | fragmented | 0.5748 | 0.9765 |
| primary | `unsafe-huffman` | whole | 0.4538 | 1.0038 |
| primary | `unsafe-huffman` | fragmented | 0.5789 | 0.9952 |
| primary | `unsafe-huffman-codebook` | whole | 0.4534 | 1.0023 |
| primary | `unsafe-huffman-codebook` | fragmented | 0.5751 | 0.9811 |
| primary | `unsafe-match-chain` | whole | 0.4511 | 0.9773 |
| primary | `unsafe-match-chain` | fragmented | 0.5755 | 0.9809 |
| primary | `unsafe-match-table` | whole | 0.4519 | 1.0041 |
| primary | `unsafe-match-table` | fragmented | 0.5840 | 0.9969 |
| primary | `unsafe-optimal-frequencies` | whole | 0.4518 | 1.0201 |
| primary | `unsafe-optimal-frequencies` | fragmented | 0.5745 | 1.0055 |
| primary | `unsafe-optimal-matches` | whole | 0.4452 | 0.9959 |
| primary | `unsafe-optimal-matches` | fragmented | 0.5672 | 0.9839 |
| primary | `unsafe-optimal-paths` | whole | 0.4519 | 0.9969 |
| primary | `unsafe-optimal-paths` | fragmented | 0.5694 | 0.9848 |
| primary | `unsafe-optimal-short-key` | whole | 0.4534 | 1.0033 |
| primary | `unsafe-optimal-short-key` | fragmented | 0.5662 | 0.9685 |
| primary | `unsafe-reference-chain` | whole | 0.4496 | 1.0000 |
| primary | `unsafe-reference-chain` | fragmented | 0.5719 | 0.9928 |
| primary | `unsafe-reference-tree` | whole | 0.4531 | 0.9943 |
| primary | `unsafe-reference-tree` | fragmented | 0.5763 | 0.9857 |
| primary | `unsafe-ring-history` | whole | 0.4618 | 0.9967 |
| primary | `unsafe-ring-history` | fragmented | 0.5867 | 0.9776 |
| primary | `unsafe-row-table` | whole | 0.4519 | 1.0094 |
| primary | `unsafe-row-table` | fragmented | 0.5701 | 0.9825 |
| primary | `unsafe-slice-history` | whole | 0.4286 | 0.9985 |
| primary | `unsafe-slice-history` | fragmented | 0.5706 | 1.0078 |
| primary | `unsafe-training` | whole | 0.4531 | 0.9879 |
| primary | `unsafe-training` | fragmented | 0.5614 | 0.9699 |
| primary | `all` | whole | 0.4559 | 0.9964 |
| primary | `all` | fragmented | 0.6195 | 0.9973 |
| repeat | `safe` | whole | 0.4546 | 1.0000 |
| repeat | `safe` | fragmented | 0.5718 | 1.0000 |
| repeat | `all` | whole | 0.4548 | 0.9959 |
| repeat | `all` | fragmented | 0.6070 | 1.0067 |

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
| `raw-small-window` | stable-whole | 256 | 3 | 640 | 640 | 3 |
| `raw-small-window` | buffered-whole | 256 | 6 | 1160 | 1160 | 6 |
| `raw-small-window` | stable-fragmented | 256 | 4 | 648 | 648 | 4 |
| `raw-small-window` | buffered-fragmented | 256 | 6 | 1160 | 1160 | 6 |
| `p0-far-dictionary` | stable-whole | 3 | 0 | 0 | 0 | 0 |
| `p0-far-dictionary` | buffered-whole | 3 | 3 | 14 | 14 | 3 |
| `p0-far-dictionary` | stable-fragmented | 3 | 1 | 8 | 8 | 1 |
| `p0-far-dictionary` | buffered-fragmented | 3 | 3 | 14 | 14 | 3 |
| `p-window-equal` | stable-whole | 1027 | 0 | 0 | 0 | 0 |
| `p-window-equal` | buffered-whole | 1027 | 3 | 1035 | 1035 | 3 |
| `p-window-equal` | stable-fragmented | 1027 | 1 | 8 | 8 | 1 |
| `p-window-equal` | buffered-fragmented | 1027 | 3 | 1035 | 1035 | 3 |
| `dictionary-to-frame-eviction` | stable-whole | 1200 | 0 | 0 | 0 | 0 |
| `dictionary-to-frame-eviction` | buffered-whole | 1200 | 3 | 1333 | 1333 | 3 |
| `dictionary-to-frame-eviction` | stable-fragmented | 1200 | 1 | 9 | 9 | 1 |
| `dictionary-to-frame-eviction` | buffered-fragmented | 1200 | 3 | 1333 | 1333 | 3 |
| `formatted-id` | stable-whole | 256 | 3 | 640 | 640 | 3 |
| `formatted-id` | buffered-whole | 256 | 6 | 1160 | 1160 | 6 |
| `formatted-id` | stable-fragmented | 256 | 4 | 648 | 648 | 4 |
| `formatted-id` | buffered-fragmented | 256 | 6 | 1160 | 1160 | 6 |
| `dictionary-treeless-four-streams` | stable-whole | 768 | 1 | 768 | 768 | 1 |
| `dictionary-treeless-four-streams` | buffered-whole | 768 | 4 | 2808 | 2808 | 4 |
| `dictionary-treeless-four-streams` | stable-fragmented | 768 | 2 | 1272 | 1272 | 2 |
| `dictionary-treeless-four-streams` | buffered-fragmented | 768 | 4 | 2808 | 2808 | 4 |
| `dictionary-repeat-sequences` | stable-whole | 512 | 1 | 59 | 59 | 1 |
| `dictionary-repeat-sequences` | buffered-whole | 512 | 4 | 1172 | 1172 | 4 |
| `dictionary-repeat-sequences` | stable-fragmented | 512 | 2 | 148 | 148 | 2 |
| `dictionary-repeat-sequences` | buffered-fragmented | 512 | 4 | 1172 | 1172 | 4 |
| `duplicate-reps-initial` | stable-whole | 3 | 0 | 0 | 0 | 0 |
| `duplicate-reps-initial` | buffered-whole | 3 | 3 | 13 | 13 | 3 |
| `duplicate-reps-initial` | stable-fragmented | 3 | 1 | 7 | 7 | 1 |
| `duplicate-reps-initial` | buffered-fragmented | 3 | 3 | 13 | 13 | 3 |
| `persistent-concatenation` | stable-whole | 512 | 6 | 1280 | 640 | 6 |
| `persistent-concatenation` | buffered-whole | 512 | 9 | 1800 | 1160 | 9 |
| `persistent-concatenation` | stable-fragmented | 512 | 7 | 1288 | 648 | 7 |
| `persistent-concatenation` | buffered-fragmented | 512 | 9 | 1800 | 1160 | 9 |
| `http` | stable-whole | 1189 | 4 | 926 | 926 | 4 |
| `http` | buffered-whole | 1189 | 7 | 3713 | 3713 | 7 |
| `http` | stable-fragmented | 1189 | 5 | 1335 | 1335 | 5 |
| `http` | buffered-fragmented | 1189 | 7 | 3713 | 3713 | 7 |
| `empty` | stable-whole | 0 | 0 | 0 | 0 | 0 |
| `empty` | buffered-whole | 0 | 0 | 0 | 0 | 0 |
| `empty` | stable-fragmented | 0 | 0 | 0 | 0 | 0 |
| `empty` | buffered-fragmented | 0 | 0 | 0 | 0 | 0 |

The source, deterministic fixtures, all retained executables, build/qualification
logs, CPU barrier, all five raw samples per case/mode, median/minimum/maximum CSV
and complete per-case ratio JSON are retained in
`evidence/2026-10-26-stable-dictionary-output-benchmarks.{json,tar.gz}`.
The primary has 4840 raw sample rows and the repeat
has 440; no unfinished sample is counted. The
recorded compiler/OS observations describe this host at preparation time, not
unrecorded environment values from earlier builds.
