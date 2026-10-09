# Dictionary, prefix, and training validation

Module 7 implements owned raw/formatted dictionaries, shared prepared metadata,
dictionary-aware encoding and decoding, single-use prefixes, retained encoder
history, and bounded raw/formatted training. Its focused conformance checks,
complete backend matrix, independent C oracle checks, external corpus replays,
and final Miri checks have passed. Final test-only FSE additions were verified
across every backend by a follow-up matrix, as detailed below. The complete
upstream test migration remains open.

The reference is facebook/zstd v1.5.7, commit
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`. The codec does not link to the C
implementation; C compilation and execution below occur only in independent
test oracles. Designs and source mappings are in
[dictionary-design.md](dictionary-design.md),
[dictionary-training.md](dictionary-training.md), and
[upstream-dictionary-plan.md](upstream-dictionary-plan.md).

## Ownership and implementation contracts

`Dictionary` owns its serialized bytes and content range. Prepared Huffman/FSE
tables and repeat offsets are validated before the object becomes usable.
`SharedDictionary` provides immutable `Arc` ownership when pointer atomics are
available and `Rc` ownership otherwise. Reusable encoder and decoder contexts
keep owned handles; no context keeps a borrowed caller buffer. Auto mode treats
short or non-magic data as raw content, while recognized formatted magic commits
to complete metadata validation. Forced Raw mode leaves matching magic as data.

Persistent selection survives reset; a prefix belongs to one standard frame.
Skippable frames do not claim it. Explicit dictionary/prefix configuration
replaces the previous selection, and mid-frame configuration is rejected
without damaging pending progress. Rust reset deliberately preserves selected
dictionaries; original C tests requiring parameter reset to clear them remain
partial and map to explicit `set_dictionary(None)` where applicable.

Dictionary content does not contribute to output counters, checksums, content
size, or output budgets. A match can span dictionary content, retained frame
output, and current block output without overwriting unread source bytes.
The selected format rule permits dictionary access while produced frame bytes
P are at most window W; P greater than W is rejected. Independent boundary
fixtures preserve the observed C implementation differences at that boundary.

The encoder keeps bounded prior output for useful matching across automatic
blocks and explicit flushes. Matches respect dictionary expiry, frame-window
bounds, reset isolation, and independent caller input/output chunk schedules.
One-shot dictionary decoding drives `Decoder`; one-shot encoding uses
`compress_frame` with the same `BlockEncoder` and history implementation as
streaming encoding. It emits the minimal dictionary-ID width. Streaming encoder
options can separately request ID omission.

Raw training uses bounded sample prefixes and deterministic frequency scoring.
Formatted construction learns a complete literal tree from sample-byte
frequencies, serializes normalized counts and two-state FSE Huffman weights,
and writes predefined sequence priors with initial repeat offsets `[1,4,8]`.
It retains the most useful content suffix under the complete serialized-size
cap and pads short explicit content to eight bytes. Automatic IDs derive from
the original content before trimming or padding. Normal dictionary preparation
revalidates every constructed object. This is an independent training algorithm;
it does not implement COVER/fastCover, compressed-sequence profiling, shrink
optimization, compression-level selection, or training workers.

The new `unsafe-dictionary` and `unsafe-training` features independently replace
checked dictionary-content reads and training-frequency access. Both retain
runtime bounds/invariants at their safe interfaces and local safety proofs.
Existing match, ring, and entropy table backends retain their own flags.
Temporary bit buffers, histogram arrays, normalized counts, and tree nodes are
owned values using checked indices; they introduce no artificial unchecked
backend. Core modules remain compatible with `no_std` plus `alloc` and explicit
fallible allocation; custom allocator parameterization remains future work.

## Focused conformance evidence

| Area | Completed evidence | Qualification |
| --- | --- | --- |
| Dictionary parser | 11 unit tests; exact HTTP/fixed-training metadata, malformed repeat/header input, Raw/Auto/Formatted selection and checked content access | Immutable validated handles replace C object construction and borrowed-pointer lifetimes |
| Decoder and prefix behavior | 40 integration tests through one-shot and fragmented streaming APIs | Original C parameter/reset variants and broader random campaigns remain separately mapped |
| Dictionary encoder | 24 regular tests; independent C decoding of 96 streamed and 20 one-shot frames | No claim of matching C levels or encoded bytes |
| Retained encoder matching | Independent regressions require useful cross-block matches, a match crossing a short flush, and dictionary-to-frame/current-output crossing | Exact output and effectiveness assertions supplement ordinary round trips |
| Raw training | Four regular tests and 16 held-out source/dictionary pairs checked in both C/Rust directions | Independent C compressed bytes decrease from 2,263 to 609 on that workload |
| Formatted training | Seven regular tests; independent C accepts eight Rust-built dictionaries and 36 source/dictionary pairs in both directions | Six explicit IDs cover every field width; empty/custom/learned content and held-out bytes are exercised |
| Generated metadata | Independent C accepts 437 FSE weight streams and 68 complete Huffman descriptions | Weight streams include odd/even termination, zero-bit updates and bounded alphabets; this does not port every entropy encoder API |
| Normalized-count target | 512 byte-identical official headers, 10,241 deterministic target calls, safe/all-feature replay of all 79 inputs/908 bytes | Complete `fse_read_ncount.c` producer/writer/readback/suffix contract; other entropy API claims stay separate |
| Training sample selector | Original `FUZZ_train` sample loop independently agrees on 36 cases and all 3,600 selected ranges, including remaining producer positions | The shared selector is ready; training-dependent target replays are tracked below |

The formatted trainer preserves the exact `fuzzer.c:2665` cyclic construction
input: 1 MiB of index-modulo-256 bytes, 128 samples of 8 KiB and a 16 KiB capacity.
Its success assertion, nonzero generated ID, nonempty metadata and usable
dictionary now run. The generic original CNBuffer training assertion also passes five exact official
5 MiB source campaigns with the same first-1-MiB sample layout and 16 KiB cap.
Both Rust and C recover all 26,214,400 source bytes, and C independently agrees
on dictionary IDs/header sizes. The original level-4 compression setup and
COVER/fastCover-specific builders remain separately incomplete. On 16 independent
held-out messages, official C compression totals 715 bytes with the Rust-built
formatted dictionary versus 2,288 without it. These workload-specific byte
counts measure effectiveness, not throughput or general compression ratios.

The formatted public suite passed safe, all-feature, and no-default-feature
tests; its optional C test was explicitly executed in safe and all-feature
builds. Focused strict Clippy and no_std compilation with all nine unsafe
features also passed. The complete repository matrix and subsequent focused
checks below establish the final backend coverage.

## Fixtures and external corpora

The dictionary collection contains 48 reproducible files backed by 123
independent C oracle outcomes. Exact expected output accompanies successful
frames. The collection covers P=W-1/W/W+1, dictionary-to-frame eviction,
out-of-bounds dictionary access, trained/prepared metadata, duplicate repeat
offsets, first-block treeless 1X/4X literals, repeat sequence tables, omitted IDs,
persistent concatenation and prefix use once. Existing official HTTP and literal
regression dictionaries are reused rather than copied into a second collection.

The generic local audit passed 311 files across seven fixture collections.
The entire external snapshot passed offline identity and extraction verification:
15 archive pins, 136,439 files and 2,682,880,516 unpacked bytes. The combined
collection limit is 4 GiB; every archive retains a 1 GiB ceiling and its own
pinned-size bound. These are rolling release assets, independently pinned on
2026-10-08/09, with no claim that their bytes originated at the v1.5.7 tag.

| Dictionary corpus | Inputs | Current replay evidence |
| --- | ---: | --- |
| `dictionary_loader` | 2,271 | Safe and all-feature release: 1,919 exact round trips and 352 formatted parse rejections; no skipped files |
| `raw_dictionary_round_trip` | 13,876 | Safe and all-feature release: every exact round trip passes; no skipped files |
| `dictionary_decompress` | 12,063 | Safe/all-feature release: 11,768 trained, 295 empty fallbacks, 20 bounded successful decodes and 12,043 source rejections |
| `dictionary_round_trip` | 30,925 | Safe/all-feature release: 30,722 trained, 203 empty fallbacks, all exact round trips |
| `dictionary_stream_round_trip` | 24,223 | Safe/all-feature release: 23,821 trained, 402 empty fallbacks, all exact round trips |

All five adapters preserve selected source bytes and supported mode/prefix
contracts; no input is skipped. Trained targets preserve the original 100-sample
selection loop and substitute independent Rust training, falling back to an empty
dictionary only on `TrainingFailed`. Other implementation/allocation errors fail
the test. C fastCover, by-reference lifetimes, advanced random parameters and
external sequence producers remain partial; omitted advanced selectors shift
later adapted schedules explicitly. The trained adapters completed 67,211 target
inputs per backend, or 134,422 safe/all-feature replays. The dictionary streaming
archive shares ordinary streaming bytes but passed its own distinct target.

## Memory-model and final validation evidence

Focused all-feature Miri checks passed all three `dictionary_train::tests`,
the near-flat Huffman-weight serialization case, and the unsafe-training
frequency count/cover/overflow case. Final all-feature Miri runs also passed
checked dictionary-content access including `usize::MAX`, owned/shared dictionary
lifetime without a second content copy, and retained match candidates crossing
input boundaries while respecting the advertised window. Each final run had
one passing test and no failures. These are targeted memory-model checks, not
an exhaustive proof.

`sh scripts/check.sh` completed successfully: twelve complete repository suite
runs covered safe Rust, each of nine unsafe features, all features in debug,
and all features in release. Eleven no_std checks covered safe Rust, each
individual unsafe feature, and the combined unsafe build. Formatting, strict
Clippy for safe/all-feature builds, source/fixture audits, and all three standalone
examples in `docs/dictionary-usage.md` also passed.

Test-only FSE additions landed while that matrix was running. The recorded
counts therefore differ by checkpoint:

| Configurations in the complete matrix | Passed per suite | Explicitly ignored per suite |
| --- | ---: | ---: |
| Safe, `unsafe-byte-cursor`, `unsafe-bitstream`, `unsafe-history`, `unsafe-ring-history` | 369 | 21 |
| `unsafe-fse`, `unsafe-huffman` | 376 | 21 |
| `unsafe-match-table`, `unsafe-dictionary`, `unsafe-training`, all features debug, all features release | 376 | 22 |

The safe run additionally executed the three standalone Markdown examples;
those are separate from its 369 repository tests. No run reported a test failure.
The final normalized-count changes then passed a twelve-configuration focused
matrix: all ten final entropy tests under safe, each of nine unsafe features,
all features and no-default-features. Std configurations retained two explicit
external ignores; the no-default-feature run had none. Both 79-file normalized-count
corpus replays ran separately in safe/all-feature release builds. This follow-up
covers the final additions on configurations whose complete-suite run preceded
them; it does not relabel the earlier runs as having the later test count.
[Exact FSE commands and log hashes](evidence/2026-10-09-fse-target.json) preserve
that evidence. No production source changed during these test-only checkpoints.

`ZSTD_UPSTREAM=/tmp/zstdoxize-upstream-1.5.7 sh scripts/check_upstream_oracles.sh`
also completed successfully. It reran every fixture importer/generator in `--check`
mode and verified all 311 fixture files across seven collections without rewriting
the checked-in fixtures. All 14 explicitly selected oracle tests passed in
release builds with all features, including 4,096 fresh generated frames, 30,000 one-shot stress cycles, the complete large
window-distance sweep, streamed and one-shot dictionary C decoding, raw/formatted
training, all five exact CNBuffer campaigns, generated entropy headers, and the
original training sample selector. The stress test's 252.38-second elapsed time
is a test-run observation, not a throughput comparison.

[Final matrix, oracle and Miri evidence](evidence/2026-10-09-dictionary-validation.json)
preserves commands, result counts and verified SHA-256 hashes of the external
local logs. The initial inventory audit inside those long-running scripts preceded
the final coverage mapping; the subsequent source-backed audit validates the
current counts recorded below. The benchmark source/input fingerprint remained
unchanged throughout the final checks.

## Memory and performance checkpoint

Benchmark plumbing is complete for 15 deterministic workloads across 11 feature
configurations. Module 7 adds dictionary content access, persistent dictionary
encoder/decoder contexts, and bounded raw training. Smoke mode passed identical
complete outputs and metadata for safe, unsafe-dictionary, unsafe-training and
all-feature configurations without collecting timings.

The timed comparison completed with identical sizes, full-output checksums,
workload metadata and source fingerprints across all 11 configurations. It used
Rust 1.89.0 on `aarch64-apple-darwin`, seed `5eedd15c0dec2026`, and five samples
of at least 150 ms per workload. The source/input SHA-256 remained
`bf2b8bb9bb67e1c315c43348f2351b7e40ac179a09f8bd70276f8121f21039ee`.
Exact CPU model access was unavailable; no processor model is inferred.

| Workload and backend | Safe ns/byte | Selected ns/byte | Relative throughput |
| --- | ---: | ---: | ---: |
| Dictionary content, `unsafe-dictionary` alone | 1.0205 | 1.0064 | 1.014x |
| Raw training, `unsafe-training` alone | 9.3851 | 9.4417 | 0.994x |
| Dictionary encoder, all features | 3.4845 | 3.3110 | 1.052x |
| Dictionary decoder, all features | 4.3854 | 4.4677 | 0.982x |
| Raw training, all features | 9.3851 | 9.4979 | 0.988x |

The two newly isolated unsafe backends show small differences on these workloads,
including a slight training regression. These workstation observations do not
establish a general benefit, and safe Rust remains the default. Dictionary
encoding uses 32,768 dictionary-derived source bytes and produces a 27-byte frame
in every build. HTTP dictionary decoding consumes 64 official frames and restores
76,096 bytes. Raw training admits at most 65,536 bytes from 256 samples and
returns 2,048 content bytes with full-output FNV-1a `b3a3cc2c22959aae`.
Formatted-training throughput is not measured by that raw-training workload.

Encoder retained capacity plateaus at 1,483,361 bytes between four and 64 input
blocks. Decoder capacity remains 509,902 bytes between four and 64 concatenated
frames, and 264,107 bytes as a single frame grows from 512 KiB to 8 MiB. With the
selected prepared dictionaries, reset retains 1,525,990 encoder bytes and 17,099
decoder bytes. These values include each context's selected dictionary once;
they exclude caller buffers, temporary scratch, inline state, allocator overhead
and peak process memory. They establish bounds for these workloads, not an
allocation-call or universal memory limit.

Complete medians, sample ranges and machine metadata are recorded with the
coordinator's [benchmark report](benchmarks.md) and the complete
[Module 7 measurement snapshot](measurements/2026-10-09-dictionaries/summary.md).

## Remaining upstream coverage

The mapped source catalog currently has 418 test-related sources, 221 explicit
non-test exclusions and 615 named groups. After the final dictionary and FSE mappings,
46 groups are ported, 102 partial, 452 blocked and 15 non-applicable. Source
statuses are 14 ported, 33 partial, 150 blocked, five planned and 216
non-applicable. These counts will change only when executable evidence or an
explicit source-based classification changes.

Original strategy/level matrices, training optimization, automatic multi-DDict
selection, C workspace/allocator/context-copy contracts, workers, large-index
stress, legacy formats, CLI behavior and other adapters remain open in the
main inventory. The success of Module 7's supported contracts cannot satisfy
`scripts/audit_upstream_tests.py --require-complete` by itself.
