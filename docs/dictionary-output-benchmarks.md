# Dictionary-backed caller-owned output measurements

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
Dictionary preparation is outside clocks. The direct entrypoint borrows its dictionary; the owned comparison clones its
handle inside the timed call.

The 11 workloads are: `raw-small-window`, `p0-far-dictionary`, `p-window-equal`, `dictionary-to-frame-eviction`, `formatted-id`, `dictionary-treeless-four-streams`, `dictionary-repeat-sequences`, `duplicate-reps-initial`, `persistent-concatenation`, `http`, `empty`.
The empty workload is excluded from geometric means and retained separately in
raw data. Per-case medians have equal weight; these are latency ratios, not a
byte-weighted throughput average. Ratios below one mean lower observed latency.

## All-backend median-latency comparisons

| Campaign | Backend | Schedule | direct / owned | direct / safe direct |
| --- | --- | --- | ---: | ---: |
| primary | `safe` | complete input | 0.2962 | 1.0000 |
| primary | `unsafe-bitstream` | complete input | 0.2980 | 0.9823 |
| primary | `unsafe-byte-cursor` | complete input | 0.2974 | 0.9968 |
| primary | `unsafe-dictionary` | complete input | 0.2930 | 0.9933 |
| primary | `unsafe-fse` | complete input | 0.2936 | 1.0011 |
| primary | `unsafe-fse-encoder` | complete input | 0.2972 | 0.9898 |
| primary | `unsafe-history` | complete input | 0.2978 | 0.9892 |
| primary | `unsafe-huffman` | complete input | 0.2927 | 0.9774 |
| primary | `unsafe-huffman-codebook` | complete input | 0.2943 | 0.9818 |
| primary | `unsafe-match-chain` | complete input | 0.2951 | 0.9778 |
| primary | `unsafe-match-table` | complete input | 0.2983 | 0.9915 |
| primary | `unsafe-optimal-frequencies` | complete input | 0.2957 | 0.9966 |
| primary | `unsafe-optimal-matches` | complete input | 0.3001 | 1.0008 |
| primary | `unsafe-optimal-paths` | complete input | 0.2986 | 1.0041 |
| primary | `unsafe-optimal-short-key` | complete input | 0.2943 | 0.9965 |
| primary | `unsafe-reference-chain` | complete input | 0.2919 | 1.0004 |
| primary | `unsafe-reference-tree` | complete input | 0.2920 | 0.9829 |
| primary | `unsafe-ring-history` | complete input | 0.2961 | 0.9903 |
| primary | `unsafe-row-table` | complete input | 0.2933 | 0.9854 |
| primary | `unsafe-slice-history` | complete input | 0.2932 | 0.9744 |
| primary | `unsafe-training` | complete input | 0.2932 | 0.9779 |
| primary | `all` | complete input | 0.2906 | 0.9707 |
| repeat | `safe` | complete input | 0.2971 | 1.0000 |
| repeat | `all` | complete input | 0.2915 | 0.9794 |

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
| `raw-small-window` | direct | 256 | 3 | 640 | 640 | 3 |
| `raw-small-window` | owned | 256 | 7 | 1416 | 1160 | 7 |
| `p0-far-dictionary` | direct | 3 | 0 | 0 | 0 | 0 |
| `p0-far-dictionary` | owned | 3 | 4 | 22 | 22 | 4 |
| `p-window-equal` | direct | 1027 | 0 | 0 | 0 | 0 |
| `p-window-equal` | owned | 1027 | 4 | 2062 | 2062 | 4 |
| `dictionary-to-frame-eviction` | direct | 1200 | 0 | 0 | 0 | 0 |
| `dictionary-to-frame-eviction` | owned | 1200 | 4 | 2533 | 2533 | 4 |
| `formatted-id` | direct | 256 | 3 | 640 | 640 | 3 |
| `formatted-id` | owned | 256 | 7 | 1416 | 1160 | 7 |
| `dictionary-treeless-four-streams` | direct | 768 | 1 | 768 | 768 | 1 |
| `dictionary-treeless-four-streams` | owned | 768 | 5 | 3576 | 2808 | 5 |
| `dictionary-repeat-sequences` | direct | 512 | 1 | 59 | 59 | 1 |
| `dictionary-repeat-sequences` | owned | 512 | 5 | 1684 | 1625 | 5 |
| `duplicate-reps-initial` | direct | 3 | 0 | 0 | 0 | 0 |
| `duplicate-reps-initial` | owned | 3 | 4 | 21 | 21 | 4 |
| `persistent-concatenation` | direct | 512 | 6 | 1280 | 640 | 6 |
| `persistent-concatenation` | owned | 512 | 11 | 2568 | 1416 | 10 |
| `http` | direct | 1189 | 4 | 926 | 926 | 4 |
| `http` | owned | 1189 | 8 | 4902 | 3976 | 8 |
| `empty` | direct | 0 | 0 | 0 | 0 | 0 |
| `empty` | owned | 0 | 0 | 0 | 0 | 0 |

The source, deterministic fixtures, all retained executables, build/qualification
logs, CPU barrier, all five raw samples per case/mode, median/minimum/maximum CSV
and complete per-case ratio JSON are retained in
`evidence/2026-10-26-dictionary-output-benchmarks.{json,tar.gz}`.
The primary has 2420 raw sample rows and the repeat
has 220; no unfinished sample is counted. The
recorded compiler/OS observations describe this host at preparation time, not
unrecorded environment values from earlier builds.
