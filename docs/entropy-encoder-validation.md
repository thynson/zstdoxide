# Entropy encoder and bounded strategy validation

Module 8 adds Huffman literal encoding, adaptive and reused sequence entropy,
transactional repeat offsets, and the independent `Greedy`, `Chain`, and `Lazy`
compression strategies. This report records implemented contracts and verified
checkpoints; the complete official test migration remains open. The reference
is facebook/zstd v1.5.7, commit
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`. C runs only in external test oracles.

## Ownership and publication

The forward bit writer owns initialized bytes and a bounded bit reservoir.
Canonical Huffman codebooks own 256 initialized entries, explicitly mark absent
symbols, and accompany their exact serialized description. Inline literal
tables are limited to eleven bits; prepared dictionary and standalone tables
may use twelve. Literal selection compares complete raw, RLE, new-table, and
reused-table section costs. The standalone test adapter additionally covers
requested depths 1 through 12 and sources through 256 KiB.

Inverse FSE tables own symbol availability, initial states, and inverse
transitions. Forward repeat planning checks every distance and zero-literal
special case before reverse sequence encoding. Each sequence channel chooses
among predefined, RLE, new, and repeated tables using exact per-channel
description/state/transition bit costs, excluding shared framing and extra bits.
Deterministic ties prefer repeat, predefined, RLE, then a smaller new-table log.
Pending Huffman/FSE replacements and repeat triples become current only after
the compressed block is accepted. Raw and whole-block RLE fallbacks discard
pending replacements; empty sequence sections preserve prior entropy state.
Reset restores dictionary/default frame state while retaining allocated capacity.

`Chain` visits at most eight candidates per position. `Lazy` visits at most
sixteen candidates at each of the current and next positions (up to thirty-two
when both probes occur) and delays only when the next
match is longer by more than one byte. Recycled chain slots use checked tags
and older-position links. All source resolution obeys dictionary lifetime,
advertised windows, initialized block ranges, and the retained history interval.
Encoder history remains bounded by `min(window, 128 KiB)`; named strategies
do not imply upstream numeric-level, full-window-search, or optimal-parser parity.

The three new independent opt-ins are `unsafe-huffman-codebook`,
`unsafe-fse-encoder`, and `unsafe-match-chain`. Release-mode checks precede
unchecked accesses, with local safety proofs. Plain codewords, mode records,
repeat triples, and transaction markers share safe value representations.
Default codec code remains entirely safe Rust and compatible with `no_std`
plus `alloc` on Rust 1.89. Allocator parameterization belongs to Module 9.

## Independent entropy checks

| Area | Executed evidence | Limits of the comparison |
| --- | --- | --- |
| Canonical Huffman descriptions and streams | Independent C accepts all five stream groups, including sparse trees, dictionary depth twelve, 1X/4X layouts, and 48 requested-depth descriptions / 96 exact streams | Rust tree construction and cost search are independent algorithms |
| Caller-owned entropy output | Safe/all-feature C checks each compare 1,554 exact successful streams and 3,479 matching zero results across supported flag patterns | Rust failure leaves caller bytes unchanged; 1,583 C zero-result calls altered destination bytes |
| Description copying | 69 C successes and 26 C errors compared against paired Rust descriptions | Nine C errors occur where copying the preselected Rust pair fits; capacity-dependent C serializer policy is explicitly distinct |
| Huffman fuzz selectors | 4,674 exact C parameter records under both BMI layouts and 1,344 adjusted/cheap-depth cases | X1/X2, BMI, assembly, and fast-loop selectors map to the available Rust engine |
| Inverse FSE and supplied tables | All 27 predefined/RLE/new channel combinations followed by repeat: C decodes 27 frames / 54 sections | Whole-source C encoder API coverage remains partial |
| Adaptive public sequence encoding | Independent C decodes 14 frames / 19,577 bytes; transactional context checks decode seven blocks plus a dictionary-backed frame | Encoded bytes need not match C's compression choices |

The Huffman target adapter preserves the original selector order, source prefix,
capacity range, hard depth adjustment, and two-byte wrapping arithmetic in the
cheap-depth formula. It distinguishes empty, single-byte, uniform, unavailable
table, insufficient description capacity, zero stream, and successful roundtrip.
Tests record requested, adjusted, selected, and actual depths separately. The
optimal policy searches feasible serialized-size estimates: description bytes
plus `floor(weighted bits / 8)`, excluding markers and jump tables. It does not
claim to duplicate the C heuristic.

The complete pinned Huffman archive contains 833 files / 2,350,233 bytes.
Both BMI layouts under safe and all-feature release builds produce 3,332 target
executions with no skipped inputs; all 1,666 per-build branch records agree.
Every requested and actual depth 1 through 12 occurs. The corpus does not enter
the unavailable-table branch or hit the full 256 KiB source cap; normal tests
provide those witnesses. Metadata, integrity checks, outcome histograms, and
lossless per-file records are retained in
[Huffman corpus evidence](evidence/2026-10-09-huffman-corpus.json) and its adjacent
compressed JSONL record file.

[Focused checkpoint records and Miri transcripts](evidence/2026-10-09-entropy-focused-index.json)
preserve exact commands, source fingerprints and available log hashes. Historical
checks recorded only in tool output explicitly retain null log paths; no log or
measurement is reconstructed as though it had been captured originally.

## Exact dictionary lifetime campaigns

The default-profile dictionary is reproduced from the original upstream
training schedule. Its 49,152 bytes have SHA-256
`d94b5d23148b133538a478ee6498c7a161b9225b305ca9fff8a45548f0e566c2`,
ID 2,004,100,296 and a 147-byte header. Other upstream random profiles remain
separate obligations.

| Upstream group | Preserved schedule and checks | Remaining qualification |
| --- | --- | --- |
| `zstreamtest.c:1551` | All 971 original requests, 34,639,743 source bytes, 34,593 generator calls, 1,024-byte buffers, one dictionary and one frame; every decoded byte compared incrementally | Rust named strategies are single-threaded with 128 KiB search history; borrowed CDict, numeric levels, and emission of every targeted offset code are not established |
| `zstreamtest.c:1831` | Both inherited window paths reproduce the exact random-state advancement and 262,144-byte source; raw-then-compressed lifetime and full dictionary bytes are preserved | Exact default profile and supported frame behavior, with independent Rust parsing policies |
| `zstreamtest.c:1884` | 43,691 six-byte flushes and nine exact dictionary slices; all 43,700 operation/output positions, 43,692 emitted blocks and 263,298 source bytes are checked | Rust emits RLE for the repeated six-byte blocks where C emits raw blocks; later entropy remains valid |

The sequence generator independently matches each request's C length, digest,
cumulative digest, and random seed, plus 6,459 per-call comparisons with zero,
small, and original buffer capacities. The complete source XXH64 is
`d6a9522c4b14b2a5` and final random seed is `0f900e80`.

The original `:1551` context inherits explicit level-five parameters, a later
level-three request, two workers, and 4 MiB jobs. Its session-only reset does
not clear those parameters. Although the newly referenced CDict requests log 24,
the actual frame advertises log 21. Rust matches that actual window and keeps
the original decoder limit and offset schedule through log 24. The C helper
uses a separate multithreaded archive; it never replaces the ordinary oracle
library. Its optional automatic build uses a separate temporary build directory.

| Encoder for the complete `:1551` source | Frame bytes | Frame XXH64 |
| --- | ---: | --- |
| Original C setup | 350,954 | `edb8a88a3d01d60a` |
| Rust Greedy | 348,912 | `dbb1ad7ed4898eea` |
| Rust Chain | 349,055 | `e125a29d7329804b` |
| Rust Lazy | 348,567 | `3a7548eab319e174` |

The three normal campaigns pass safe, all-feature, no-default-feature, and each
of twelve independent unsafe builds: 45 complete campaigns. Safe/all-feature
C runs each decode one C baseline in Rust and three Rust frames in C, checking
103,919,229 Rust-to-C output bytes. Exact commands, frame fingerprints and log
hashes are in [sequence sweep evidence](evidence/2026-10-09-sequence-sweep.json).
These byte counts are interoperability observations, not throughput results.

## Named-strategy regression content

Every one of the 512 pinned Silesia/GitHub regression inputs is replayed through
a reused encoder for each strategy, reset between files, with bounded guarded
output and complete decoded-byte comparison. Safe/all-feature runs agree on
all frames and metadata. Total source length is 12,407,963 bytes.

| Strategy | Encoded bytes | Encoded/source | Peak reported retained bytes | Frame transcript FNV-1a |
| --- | ---: | ---: | ---: | --- |
| Greedy | 5,280,917 | 42.5607% | 3,609,357 | `3f1239fc5ab02cb8` |
| Chain | 4,945,104 | 39.8543% | 12,431,212 | `a9d6ac79b868c4d7` |
| Lazy | 4,853,648 | 39.1172% | 12,409,205 | `82b83c1b3842cb4c` |

Reported retained bytes sum owned context heap capacities observed after calls;
they exclude inline state, caller buffers, allocator overhead and transient
scratch. The source transcript is `e67d76daccd0c28a`. This is complete content replay
through the named Rust strategies; the upstream regression framework's numeric
levels, dictionary methods, CLI, tar, and CSV behavior are still partial.
[Regression evidence](evidence/2026-10-09-strategy-corpora.json) records the
archive checks, commands and per-strategy results.

## Final matrix and memory-model checks

The complete frozen matrix passed fifteen repository suite runs: safe, each of
twelve individual unsafe features, all features in debug, and all features in
release. Each suite ran 534 passing tests (289 unit, 244 integration, one crate
doctest) and retained 45 explicit ignores, totaling 8,010 successful test-case
executions across repeated configurations. The ignores are not counted as passes.
Fourteen `no_std` configurations, both strict all-target Clippy checks, formatting,
source/fixture audits, three dictionary-usage examples and two streaming-usage
examples also passed.

All 429 source/test/build inputs and both compiled usage documents remained
unchanged. The source/test fingerprint before and after was
`53c233ef8c0edd7a1f42963ae7f994b67761d67ba4e13edbf642a6912334f360`.
[Matrix evidence](evidence/2026-10-09-entropy-matrix-results.json) and adjacent
[losslessly compressed before/after manifests](evidence/2026-10-09-entropy-matrix-manifests.json)
preserve per-command counts, compiler details and hashes.
The matrix used Rust 1.89.0 on `aarch64-apple-darwin`; its 1,079.25-second runtime
is a validation observation, not a benchmark. The supplementary streaming
examples bring the complete validation time to 1,080.15 seconds.

The CI Miri follow-up occurs after that frozen matrix. It changes only the
workflow and a test launcher; production and test implementations remain the
same. Its separately recorded checks must not be attributed to the earlier
matrix. All seventeen workflow selectors match real tests. Six focused Miri
commands passed eight tests, including the relocated writer and all three new
unsafe access families; a deliberately missing selector returned failure.
[CI follow-up evidence](evidence/2026-10-09-entropy-ci-miri.json) records those
checks without claiming the entire Miri workflow ran again.

The final independent oracle script completed successfully in release mode with
all features: 24 Cargo commands selected 35 passing oracle tests. All source
hashes and generated fixtures were rechecked, including 96 decodecorpus frames,
16 headers, 512 FSE vectors, 30 Huffman vectors, 20 window-distance frames, and
48 dictionary fixtures backed by 123 oracle outcomes. The run includes 30,000
intensive one-shot cycles, fresh generated-frame interoperability, all entropy
helpers, and the three original dictionary lifetime campaigns. The complete
local fixture audit covers 312 files across eight collections.
[Oracle commands and log hash](evidence/2026-10-09-entropy-oracles.json) retain
all 35 result counts; the 413.16-second elapsed time includes compilation and
concurrent normal testing and is not a performance comparison.

Focused Miri checks already passed inverse-FSE accessor bounds, forward writer
operations, chain access at extreme indices, cyclic slot reuse and stale-link
rejection, and lazy overlap at full-width coordinates. Huffman checks covered
24 direct/FSE trees spanning depths 1 through 12, and all seven target outcomes
under both BMI layouts. These targeted checks supplement ordinary malformed
input and C interoperability tests; they are not exhaustive memory-safety proof.

A separate static review found no actionable correctness or memory-safety issue
in candidate publication/discard, history preflight, sequence-table ownership,
stream error poisoning/reset, chain/lazy bounds, or the three new unsafe access
families. It also checked the inherited `:1551` C setup. That review did not
inject allocation failures or execute a native 32-bit build.

## Backend performance and retained memory

The full timed comparison passed fourteen configurations and twenty-four
workloads with identical deterministic output, sizes and memory metadata.
Five samples of at least 150 ms were collected per workload on Rust 1.89.0 /
Apple AArch64 after other compilation and tests stopped. The benchmark's scoped
source/input fingerprint remained
`d7730800b32fdabe1ecd4a225a8240b91bbd894dda206fea5058045de79467ad`.
This fingerprint covers benchmark inputs and differs intentionally from the
broader matrix fingerprint that also includes scripts, CI and other tests.

The new independent unsafe FSE encoder and Huffman codebook accessors measured
0.928x and 0.978x relative throughput; the chain accessor measured 1.011x.
The first two were slower and the third difference is small. Safe defaults and
explicit opt-ins remain appropriate. All-feature results include mixed effects;
they do not identify a single backend's contribution or establish general gains.

The three strategies encode the 99,328-byte compressible source to 1,198 / 1,203 /
1,206 bytes, with safe timings 3.0672 / 4.4887 / 6.0918 ns per input byte. On the
8,320-byte ambiguous source with one identical 8,960-byte dictionary, they encode
403 / 369 / 279 bytes in 11.5277 / 24.9433 / 21.6123 ns per input byte. Chain and
Lazy improve this second source's size at a time and memory cost; the first
source shows why a higher search budget is not a universal improvement.

Observed retained-capacity plateaus are 263,841 decoder bytes for one frame
growing from 512 KiB to 8 MiB of output, 509,902 bytes for four versus sixty-four
concatenated frames, and 1,510,177 Greedy encoder bytes for four versus sixty-four
128 KiB input blocks. Inline state, caller buffers, allocator overhead and
transient scratch are excluded. Full ranges, context capacities, exact inputs,
and per-configuration output hashes are in the
[measurement summary](measurements/2026-10-09-entropy/summary.md),
[benchmark discussion](benchmarks.md), and
[execution/file-hash record](evidence/2026-10-09-entropy-benchmarks.json).

## Remaining upstream work

The source-backed inventory contains 418 test-related sources and 615 named
case groups. Fourteen sources are ported, 34 partial, 149 blocked, five planned,
and 216 non-applicable; cases are 47 ported, 104 partial, 449 blocked and 15
non-applicable. The 188 unfinished behavioral source units keep
`scripts/audit_upstream_tests.py --require-complete` unsatisfied. Its
[expected nonzero result](evidence/2026-10-09-entropy-migration-gate.json)
is preserved separately from the successful structural and source-hash audits.

Outstanding work includes advanced parameter/reset and stable-buffer controls,
allocator/workspace contracts, numeric compression levels, full-window and
long-distance matching, training optimizations, threading, sequence producers,
legacy decode success, CLI and format adapters, and the remaining target-specific
corpus adapters. The completed entropy module is not a complete replacement
for the upstream library or programs.
