# Recorded backend comparisons

The [allocator-aware benchmark report](allocator-benchmarks.md) records the
Module 9 measurements, accounting boundaries, and raw validation evidence.

## Module 8 entropy and strategy measurements

The 2026-10-09 entropy run used Rust 1.89.0 on `aarch64-apple-darwin`, release
overflow checks, five samples of at least 150 ms, fourteen configurations and
twenty-four workloads. Safe, each of twelve independent unsafe features, and
all features produced identical per-workload fingerprints, complete output
hashes, sizes and retained-capacity metadata. Source and benchmark inputs stayed
unchanged throughout. The [complete summary and raw measurements](measurements/2026-10-09-entropy/summary.md)
include every earlier backend and the three new access families plus six named
strategy/source pairs.

| New feature alone | Workload | Safe ns/unit | Feature ns/unit | Relative throughput |
| --- | --- | ---: | ---: | ---: |
| `unsafe-fse-encoder` | Inverse FSE transition | 1.1416 | 1.2306 | 0.928x |
| `unsafe-huffman-codebook` | Canonical code lookup | 0.7454 | 0.7618 | 0.978x |
| `unsafe-match-chain` | Candidate lookup | 2.3919 | 2.3657 | 1.011x |

The FSE and codebook opt-ins were slower in isolation; the chain difference is
small. These sequential workstation measurements do not establish a general
benefit for the new unsafe families. Safe Rust remains the default. All-feature
results include interactions and noise: one-shot text decoding improves from
1.1892 to 0.9460 ns/byte (1.257x), while dictionary compression changes from
4.9111 to 5.6150 ns/byte (0.875x). The complete raw ranges are retained; no C
throughput or universal speedup is claimed.

| Strategy | Source | Safe ns/input byte | Encoded bytes | Reset retained bytes |
| --- | --- | ---: | ---: | ---: |
| Greedy | 99,328 compressible bytes | 3.0672 | 1,198 | 1,351,457 |
| Chain | Same compressible source | 4.4887 | 1,203 | 10,259,371 |
| Lazy | Same compressible source | 6.0918 | 1,206 | 10,266,865 |
| Greedy | 8,320 ambiguous bytes | 11.5277 | 403 | 1,425,572 |
| Chain | Same ambiguous source | 24.9433 | 369 | 12,959,340 |
| Lazy | Same ambiguous source | 21.6123 | 279 | 12,942,794 |

The ambiguous workload uses the same 8,960-byte raw dictionary for all three
strategies; the compressible workload has no dictionary. The named strategies
are independent Rust algorithms, not upstream numeric levels. Greedy wins both
time and size on this compressible input. Chain and Lazy spend more time and
memory to improve size on the ambiguous input. These workload-specific tradeoffs
do not imply that a higher search budget always improves compression. The
complete 512-input regression content comparison is recorded separately in
[the upstream regression adaptation](../tests/upstream/REGRESSION.md).

Memory trials show the decoder retaining 263,841 bytes for both 512 KiB and
8 MiB of single-frame output, and 509,902 bytes for both four and sixty-four
concatenated frames. Greedy encoding retains 1,510,177 bytes for both four and
sixty-four 128 KiB input blocks. These are observed context heap capacities,
excluding inline state, caller buffers, allocator overhead and transient scratch;
they are not universal process-memory limits. The persistent dictionary
decoder/encoder retains 17,099 / 1,827,319 bytes after reset.

The measured source/input SHA-256 is
`d7730800b32fdabe1ecd4a225a8240b91bbd894dda206fea5058045de79467ad`,
at revision `84fef7606e12898f2fc53b2fd8158dd1c42b676f`. Other agents stopped
compilation and tests during timing. CPU model access was unavailable; the
compiler, host, timestamp and dirty documentation paths are recorded in the
environment file. [Benchmark execution evidence](evidence/2026-10-09-entropy-benchmarks.json)
includes the successful script status and hashes of every measurement file.

## Module 7 dictionary and training measurements

The 2026-10-09 run used Rust 1.89.0 on `aarch64-apple-darwin`, release overflow
checks, five samples of at least 150 ms, eleven configurations and fifteen
workloads. The configurations are safe defaults, each of the nine unsafe
features alone, and all features together. All deterministic fingerprints,
whole-output checksums, sizes, and retained-capacity metadata matched. The
[complete summary and raw samples](measurements/2026-10-09-dictionaries/summary.md)
include all earlier workloads as well as the four new dictionary/training cases.

| Workload | Safe ns/byte | Selected unsafe ns/byte | Relative throughput |
| --- | ---: | ---: | ---: |
| Dictionary content, dictionary feature alone | 1.0205 | 1.0064 | 1.014x |
| Raw training, training feature alone | 9.3851 | 9.4417 | 0.994x |
| Dictionary compression, all features | 3.4845 | 3.3110 | 1.052x |
| Prepared dictionary decompression, all features | 4.3854 | 4.4677 | 0.982x |

The two new unsafe accessors show little difference on these workloads; the
training feature is slightly slower in this run. These sequential workstation
measurements include noise and do not establish a universal improvement. Unsafe
features remain opt-in. Compression/decompression timings reuse contexts and
include reset, while raw training includes scratch allocation and the complete
selection algorithm. Formatted-training throughput is not measured by that raw
selection workload.

Dictionary compression encodes 32,768 source bytes to 27 bytes, compared with
2,074 bytes for the same source without the dictionary. Prepared HTTP decoding
processes 64 frames containing 76,096 output bytes from 27,328 encoded bytes.
The raw trainer observes 65,536 bytes from 256 samples totaling 131,072 bytes,
uses 64-byte segments and a 2,048-byte output cap, and returns exactly 2,048
bytes with full-output FNV-1a `b3a3cc2c22959aae` in every configuration.

Persistent prepared dictionary decoder/encoder contexts retain 17,099 and
1,525,990 heap-capacity bytes after reset, including each dictionary once per
context report. Ordinary streaming memory trials retain 264,107 decoder bytes
for both 512 KiB and 8 MiB of output, and 1,483,361 encoder bytes for both four
and 64 blocks. The encoder increase of 131,072 bytes from Module 6 corresponds
to the new retained-output history ring. These measurements exclude inline
state, transient scratch, caller buffers, allocator overhead and shared-owner
deduplication across separate contexts.

The source SHA-256 is
`bf2b8bb9bb67e1c315c43348f2351b7e40ac179a09f8bd70276f8121f21039ee`,
with revision `8f55d3df3b83925c246c6fd4e7e6ae97b59264ec`. The benchmark script
verified that core source, benchmark code, and consumed fixtures did not change
during the run. Other agents paused CPU-intensive validation for the timed run.
CPU model access was unavailable; compiler/host metadata is preserved.

## Module 6 streaming measurements

The streaming run on 2026-10-08 used Rust 1.89.0 on `aarch64-apple-darwin`,
release overflow checks, five samples of at least 150 ms, nine configurations,
and eleven workloads. All output fingerprints, full-output FNV checksums,
lengths, and retained-capacity metadata matched across configurations. The
[complete summary and raw measurements](measurements/2026-10-08-streaming/summary.md)
include ranges, environment, and the source fingerprint.

| Workload | Safe ns/byte | Selected unsafe ns/byte | Relative throughput |
| --- | ---: | ---: | ---: |
| Ring overlap, ring feature alone | 0.9486 | 1.0033 | 0.945x |
| Ring overlap, all features | 0.9486 | 0.7690 | 1.234x |
| Streaming decode, all features | 1.6338 | 1.4634 | 1.116x |
| Streaming encode, all features | 1.7546 | 1.6325 | 1.075x |

The independent ring feature was slightly slower in this run. Other feature
interactions and workstation noise prevent assigning all-feature gains to that
one data structure. Every unsafe backend remains opt-in. Streaming measurements
reuse contexts and caller buffers; one-shot timings include their ordinary
allocation, so differences between those workloads are not backend speedups.

Retained decoder capacity stayed at 264,107 bytes while one frame grew from
524,288 to 8,388,608 output bytes. Concatenated-frame trials retained 509,902 bytes
for both four and 64 frames (23,700,864 bytes of output). The encoder retained
1,352,289 bytes after both four and 64 blocks (8,388,608 input bytes). These
measurements count owned context capacities after calls; they exclude temporary
literal/table construction, inline state, caller buffers, and allocator overhead.

The measurement source SHA-256 was
`e191c5778952b1be18cd584b05fd4235508ddd8986aa35079503a5cadab56df5`,
with parent commit `c94efe4` and the recorded benchmark additions present.
A subsequent fix only changes rejection of decoder processing after sealed EOF;
that error path was not exercised by the benchmark's timed operations. Results
belong to the recorded snapshot, not a claim of an identical later source hash.

## Earlier one-shot milestone


Measured on 2026-10-08 with Rust 1.89.0, `aarch64-apple-darwin`, release settings,
and overflow checks enabled. Each workload used five samples of at least 150 ms;
the tables report medians. The safe build, each of six features alone, and all
features used identical input and compiler settings. Exact CPU-model access was
unavailable under the local permissions; host/target metadata is retained.

| Data structure feature alone | Safe ns/unit | Feature ns/unit | Relative throughput |
| --- | ---: | ---: | ---: |
| `unsafe-byte-cursor` | 0.4924 | 0.5149 | 0.956x |
| `unsafe-bitstream` | 1.3950 | 1.4308 | 0.975x |
| `unsafe-fse` | 0.7471 | 0.7491 | 0.997x |
| `unsafe-huffman` | 6.0675 | 6.2373 | 0.973x |
| `unsafe-history` | 0.6258 | 0.3801 | 1.646x |
| `unsafe-match-table` | 1.0374 | 1.0119 | 1.025x |

Units are bytes except FSE transitions and match-table candidates. Tables are
constructed outside lookup timings; history/Huffman output allocation is
included. See [the benchmark design](benchmark-design.md) for exact workloads.

| Complete operation / all features | Safe ns/byte | All features ns/byte | Relative throughput |
| --- | ---: | ---: | ---: |
| Compression, mixed deterministic input | 1.7035 | 1.7200 | 0.990x |
| Decompression, official text fixture | 1.1925 | 0.9136 | 1.305x |
| Overlapping history expansion | 0.6258 | 0.3746 | 1.671x |

The compression input was 99,328 bytes and encoded to 1,339 bytes in every
configuration. The decompression workload uses an official 7,124-byte frame
containing 370,326 decoded bytes. These are deliberately specified workloads,
not a general compression-ratio comparison with upstream zstd.

The history backend showed the clearest benefit. Other isolated differences are
small, and several unsafe paths were slower in this run. All features improved
text-fixture decompression while compression was effectively unchanged. The safe
implementation remains the default. No upstream C throughput measurement or
general performance-parity claim is made.

The run contains outliers: all-feature compression ranged from 1.6674 to 2.5146
ns/byte; FSE lookup with only `unsafe-huffman` ranged from 0.7472 to 1.1698
ns/transition. Unrelated feature effects should not be inferred from those
samples. The sequential workstation run establishes observations on these
inputs, not statistically controlled universal speedups.

The script verified source stability and matching workload sizes, sampled
fingerprints, and full-output FNV-1a checksums across all configurations. The full
checksums and compression roundtrip occur outside timing. Noncryptographic
checksums supplement the independent full-output interoperability tests.

Reproduce with `bash scripts/benchmark.sh`. The measured source revision was
`0712a828751fc42d4f1d979c7e2081e289d84e8f`; its source SHA-256 was
`365a5ce09602eb5bf58dea6b06d8ca2cacf36f2ddf55846c9d638c89103d3459`.

Raw medians, sample ranges, workload sizes, checksums, compiler metadata, and
the generated summary are in [measurements/2026-10-08](measurements/2026-10-08/summary.md).
