# Development progress

Detailed completed work items are preserved in Git history. `TODO.md` contains
only active module checklists and the remaining roadmap.

## Foundation

Project instructions, the pinned v1.5.7 data structure inventory, architecture,
and a dependency-free Rust 2024 crate are complete. Both safe default and all
feature combinations compile without `std` using Rust 1.89.0. Codec modules are
being delivered as small work-item commits.

## Entropy primitives

Reverse bitstreams, normalized FSE distributions, bounded two-state Huffman
weights, canonical Huffman tables, and all literal section modes are implemented.
Each access backend has its own feature and checked interface. Focused tests and
the official frame corpus pass in safe and all-feature configurations.

## One-shot frame decoder

Frame/block headers, overlap-aware history, repeat offsets, sequence table reuse,
XXH64 verification, concatenation, and skippable frames are implemented. Output
and advertised-window limits are explicit. Twelve checked-in official v1.5.7
fixtures and 56 runtime oracle cases pass. This milestone excludes dictionaries,
legacy frames, and resumable streaming; full upstream parity remains open.

## Initial compressor

Checksummed frames select raw, RLE, or greedy LZ-compressed blocks. Matches use
position indices, raw literals, explicit offsets, and predefined FSE tables.
Encoding transitions are constructed by inverting the decoding relation.
Thirty-one independent official-decoder cases pass; compression ratio and speed
have not been tuned to match upstream levels. Adaptive entropy, Huffman encoding,
and cross-block matching are still open.

## Project rename

The project is named `zstdoxide`. On 2026-10-08, all 25 existing commits were
rebased from the root to correct the package name, crate references, and script
variables throughout history. All 612 historical file snapshots were checked
against the originals: only the intended textual name changed, and commit
boundaries were preserved. A local recovery branch
`codex/before-zstdoxide-rename` retains the previous history.

## Initial validation and measurement

The safe build, six individual backends, all backends, release mode, and no_std
matrix passed. Targeted Miri checks cover every unsafe access family. The local
suite contains 53 unit tests, five regular interoperability tests, two malformed
input tests, one documentation example, and two separately executed oracle tests.
See [validation.md](validation.md) for exact evidence and remaining gaps.

Every backend and whole-codec workload was measured with five timed samples on
the same source and input. Complete output sizes and FNV-1a checksums match in
all configurations. Isolated unsafe history showed 1.646x throughput; all-feature
text-fixture decompression showed 1.305x while compression was essentially
unchanged. Raw data, ranges, and limitations are recorded in
[benchmarks.md](benchmarks.md). Default builds remain entirely safe.

The initial working codec milestone is complete. Streaming, dictionaries,
advanced compression, custom allocators, and full upstream parity are not.

## Official test migration (in progress)

The pinned upstream tree now has a complete source inventory and 615 named
case-group anchors. Exact golden, inline, testcard, kernel, entropy, and
decodecorpus fixtures have reproducible imports and offline integrity checks.
The executable Rust adaptations include the deterministic upstream generators,
one-shot regressions, four public fuzz targets, entropy decoder targets, and
external regression-data content checks.

At the pre-streaming checkpoint, the suite had 140 ordinary passing tests and nine explicit opt-in/pending
tests. Extended checks include 30,000 one-shot fuzz cycles, 4,096 independently
generated frames, 15,804 fuzz corpus files, and all 512 selected Silesia/GitHub
regression input files. See [the validation report](upstream-test-validation.md)
for the exact distinction between a test, a fixture, and a corpus execution.

This module remains open. At the pre-streaming checkpoint, of the 418 test-related source units, 11 were ported,
25 partial, three planned, 162 blocked by missing capabilities, and 217 are
support/build metadata or C-specific details. These are source dispositions,
not an assertion-coverage percentage. The strict completion gate rejects the
190 unfinished behavioral source units and incomplete external target coverage.
The full-legacy desired-success test still fails with `InvalidMagic` when
explicitly run. `TODO.md` retains the required capability and test work;
completed migration checkpoints are not cleared as a completed major module.


## Module 6: bounded streaming and reusable contexts

Completed on 2026-10-08. Encoder and decoder contexts now accept caller slices,
report exact progress and frame boundaries, retain bounded block/window storage,
and support incremental checksums, Flush/Finish, pledged sizes, poisoned errors,
and reset reuse. Raw output is incremental before the complete block arrives.
The ring history has an independent safe/unsafe backend and full local proofs.
One-shot encoded bytes remained unchanged through workspace extraction/reuse.

All applicable ordinary streaming/context assertions were ported with exact
source mappings, including the two inline regressions and the full near-window
sweep through log 29. Both entire external streaming corpora passed safe and
all-feature release replay: 12,462 decompression and 24,223 round-trip inputs,
with no skipped files. Missing dictionary/parameter/stable-buffer/worker branches
remain explicitly assigned to later work rather than counted as covered.

The full local matrix passed 207 tests in each of ten runs, all nine no_std
configurations, formatting, strict Clippy, source/fixture audits, and independent
oracle checks. Targeted Miri and all-feature compressed streaming regressions
passed. The final external runner was verified separately after the matrix;
the ordinary count remains 207, with 12 deliberate ignored entry points.
Retained-memory plateaus and all nine performance configurations are recorded
in [streaming-validation.md](streaming-validation.md) and [benchmarks.md](benchmarks.md).
Unsafe ring access was slightly slower in isolation and remains opt-in.

The source inventory still has 190 incomplete behavioral source units; completing
streaming does not complete the full upstream migration. Module 7 is decomposed
in `TODO.md` before dictionary implementation begins.


## Module 7: dictionaries, prefixes, and training

Completed on 2026-10-09. Raw and formatted dictionaries now own validated content
and entropy metadata, with immutable shared handles, persistent context selection,
single-use prefixes, exact ID handling, and dictionary-to-frame crossing matches.
Encoder history retains at most 128 KiB of output for cross-block matching.
Bounded raw content selection and independent formatted training produce
interoperable dictionaries with learned literal trees and predefined sequence
priors. COVER/fastCover and advanced training controls remain later obligations.

The supported official parser, lifetime, ID, window, sample-selection, and training
contracts have explicit source mappings. All five dictionary corpora passed safe
and all-feature release replay: 83,358 inputs per backend with no skipped files.
The exact normalized-count target now includes all 512 byte-identical C headers,
10,241 deterministic calls, and both complete 79-input corpus replays.

The complete twelve-run test matrix, eleven no_std configurations, formatting,
strict Clippy, independent C oracles, fixture audits, and targeted Miri passed.
Test-only normalized-count additions landed during the full matrix; the final
suite has 376 passing tests and 22 explicit ignores, and a twelve-configuration
focused follow-up covers every new entropy assertion. Exact per-run counts and
log hashes are retained in [dictionary-validation.md](dictionary-validation.md).
Oracle checks include 30,000 stress cycles and all five exact CNBuffer campaigns.

All eleven backend configurations produced identical deterministic benchmark
outputs. Dictionary access showed a small isolated improvement, while unsafe
training was slightly slower; both remain opt-in. Throughput, sample ranges,
source fingerprints, and retained-memory plateaus are in [benchmarks.md](benchmarks.md).
The full upstream migration remains open: 14 of 418 test-related source units
are ported, 33 partial, 150 blocked, five planned, and 216 non-applicable.

The completed checklist is archived in Git history. Module 8 is decomposed in
`TODO.md` and [entropy-encoder-design.md](entropy-encoder-design.md) before
implementation. It adds Huffman literals, adaptive/reused FSE, repeat planning,
and bounded named compression strategies without upstream level-parity claims.


## Module 8: entropy encoding and bounded strategies

Completed on 2026-10-09. The encoder now selects raw/RLE/new/reused Huffman
literals and adaptive predefined/RLE/new/reused sequence distributions, plans
repeat offsets forward, and publishes entropy only for accepted compressed
blocks. Greedy, Chain and Lazy share checked dictionary/history resolution;
Chain searches eight candidates and Lazy up to sixteen at each current/next
position. Output history remains bounded by 128 KiB. The three new unsafe
families remain independent opt-ins with release-mode guards and local proofs.

The complete adapted Huffman driver replays every pinned corpus input under
both BMI layouts and safe/all-feature builds: 3,332 executions with all 1,666
per-build records identical. All 512 regression inputs pass each named strategy
under both backends. The original dictionary campaigns preserve both raw-block
RNG paths, all 43,691 six-byte flushes, and the complete 971-request / 34,639,743-
byte sequence source, with independent C decoding in both directions. Numeric
levels, borrowed C dictionary ownership and multithreading remain qualified.

Fifteen full suites each pass 534 tests with 45 explicit ignores; fourteen
no_std configurations, both strict Clippy checks, formatting, source/fixture
checks, and five additional usage-document examples pass. The complete oracle
script selects 35 passing tests, including 30,000 stress cycles. Frozen inputs
are fingerprinted. Focused Miri covers all three new unsafe families; the CI
launcher now rejects selectors that execute zero tests. These facts and precise
limitations are archived in [entropy-encoder-validation.md](entropy-encoder-validation.md).

The fourteen-configuration, twenty-four-workload performance run preserves
identical outputs and metadata. Isolated unsafe FSE encoding and Huffman
codebook access were slower; chain access showed a small difference. Safe
remains the default. Named strategies show workload-specific size/time/memory
tradeoffs, detailed in [benchmarks.md](benchmarks.md). No upstream throughput
or numeric-level parity is claimed.

The full migration gate remains open with 188 unfinished behavioral source
units. Completed Module 8 tasks are archived in Git history. Module 9's
allocation ownership, API boundaries, failure contracts and exact upstream
memory schedules are analyzed before implementation, with small items retained
in TODO. The actual Rust 1.100 distribution was unavailable (HTTP 404); a
standard-API no_std probe compiled on the installed 1.101 nightly without
feature gates. The next compiler decision must preserve that distinction.


## Module 9: allocator ownership and focused platform validation

Completed on 2026-10-09. The core now uses the standard `Allocator` and typed
`Vec<T, A>` owners throughout one-shot/streaming compression and decompression,
dictionary preparation, raw/formatted training, and temporary entropy/match
storage. Convenience APIs retain Global. The original allocator moves with
returned output; each other owner retains its exact allocator clone, without
requiring clones to be interchangeable. Borrowed non-Clone allocators and
allocator-independent `DictionaryView` attachments are supported. The default
codec remains entirely safe Rust and no_std plus alloc.

The Huffman and training heaps use checked allocator-compatible storage.
Failure sweeps cover each observed allocation in selected leaf/public workloads,
including preparation, reset recovery and final release. Unsafe access retains
runtime bounds/initialization checks and local proofs. An independent source
review found no open ownership, routing or unsafe-proof issue in its recorded
final source; this is scoped evidence, not a universal safety proof.

The actual compiler pin is `nightly-2026-10-08`, reporting Rust 1.101.0-nightly
at `1d81eb4ad9cd207e3e638bd32b17ec4fce8412a6`. The package conservatively declares
1.101; neither a stable 1.101 release nor unavailable Rust 1.100 was tested.
No unstable feature gate or bootstrap flag is used for the chosen allocation
API subset. Earlier Module 8 Rust 1.89 evidence remains historical.

The final immutable codec snapshot `8703ff4` passed 58 validation commands:
15 complete suites each had 699 passed and 50 ignored, for 10,485 passed
executions; each individual unsafe backend and their combination compiled
without std. Strict Clippy, formatting, helper tests, standalone usage examples,
pinned-source inventory integrity and all 312 imported fixtures passed. Explicit
safe/all-feature official v1.5.7 checks made 814 complete decoded-byte comparisons.
The later allocator benchmark harness had its own strict Clippy, formatting,
parser checks and 14-configuration smoke validation.

Focused platform evidence comprises 38 earlier selected Miri executions and
12 final encoder/API follow-ups across 32-bit little-endian and 64-bit big-endian
targets. The no-pointer-atomics ARM target compiled both safe/all library and
caller paths, including exact Rc selection. These are interpreted Miri and
compile-only ARM checks, not native foreign execution or firmware qualification.
The original 89,456,640-byte allocator source was also preserved and validated
with explicit seed and supported Rust strategies, with independent C decoding;
upstream numeric levels and worker schedules remain separate obligations.

The frozen allocator benchmark source at `2e12c18` produced 462 rows and 2,310
samples across 14 backends, eleven workloads and three allocator modes. Output,
allocation counts, cumulative peaks and retained capacities agreed across
backends; all tracked final drops released every payload. Global counters are
uninstrumented. The Global one-shot encode/decode medians were 12.055/1.386
ns per byte safe versus 9.724/1.120 with all features; other changes were smaller
and dictionary preparation was slightly slower. These fixed-order host-specific
observations include reported sample ranges and establish no universal speedup
or C-throughput parity.

Standard shared dictionaries retain an explicit infallible Global allocation
for their counts and inline object; borrowed views avoid that ownership layer.
Static workspaces, required-size estimators, automatic shrinking, stable caller
sessions, actual Rust 1.100 verification and broader native qualification remain
open. The complete official migration gate still reports 188 unfinished source
units and partial/blocked external targets. Module 9 does not mark that gate
complete.

Evidence and reproducible details: [final codec matrix](allocator-final-validation.md),
[failure coverage](allocator-failure-validation.md),
[platform checks](allocator-platform-validation.md),
[allocator measurements](allocator-benchmarks.md), and
[compiler/ownership boundaries](allocators.md). Completed Module 9 tasks are
removed from TODO and retained in Git history. Module 10's parameter inspection
and supported encoder lifecycle are analyzed and decomposed before implementation;
operational numeric algorithms and later roadmap work remain explicit follow-ons.

## Module 10: parameter oracle checkpoint

The first implementation prerequisite is complete on 2026-10-09. Seven pinned
TSV fixtures preserve 934 C observations: all 92 presets, 13 public bounds,
two width-bound records, 266 queries, 225 adjustment cases, 150 requested-value
operations and 186 bounded lifecycle observations. The oracle/importer retain
source attribution and hashes; neither is linked into the Rust codec.

Independent review checked public-zero versus typed-empty semantics, signed
clamping, two-pass requested resolution, a meaningful small source hint,
pending/completed operations, actual pledge failure, and cached dictionary-ID
labels. Fresh C parameter reset preserves its pledge; the planned Rust reset
restores its own defaults. C's cached ID does not prove retained ownership.

Seven focused checks passed, including exact regeneration under both `cc` and
`clang` command names and a probe-only UBSan run. These commands use the same
Apple Clang installation; no independent compiler implementation or instrumented
reference archive is claimed. Fixture integrity covers 319 files in nine
collections. An isolated audit rejects corrupted importer/input hashes,
escaping paths and altered fixture bytes. Source inventory integrity passes;
no migration statuses were promoted merely because observations were imported.

This checkpoint implements no Rust scalar API and runs no Rust codec tests.
Only 64-bit C was executed; the 32-bit maxima are explicitly macro-derived.
Fixture contracts and reproduction live in
[the collection README](../tests/upstream/parameters/README.md), with
[validation metadata](evidence/2026-10-09-parameter-oracle.json) and archived
logs. Value records, query algorithms, live configuration and reset work remain
in the Module 10 checklist.

The next checkpoint implements the six allocation-free scalar records and
atomic request setters. Thirteen focused tests pass in four safe/feature/std
configurations, both no_std library checks and strict safe/all Clippy pass,
and the two new usage examples pass under both backends. The 225 raw-input
validation outcomes, 119 scalar request-state rows and reference frame defaults
are checked with explicit exclusions for unimplemented operations. Presets,
adjustment and complete query resolution remain separate upcoming items; see
[parameter validation](compression-parameters-validation.md) and
[runnable usage](parameter-usage.md).

The independent live configuration checkpoint adds `Encoder::set_options` at
the clean pre-header boundary. Failure preserves the complete context without
poisoning it; success retains dictionary/prefix selection and allocator owners.
Safe/all encoder suites each pass 22 tests, the new boundary/ownership tests pass
four additional feature/std configurations, and 36 configured frames pass exact
independent C decoding across safe/all release builds. Default frame bytes remain
unchanged. Parameter reset and full reset remain separate work items.

The unadjusted preset foundation now matches all 92 pinned C table entries and
their selected forms, with checked tier arithmetic and negative acceleration.
Twenty-two parameter tests pass in each of four configurations; no_std, strict
Clippy and formatting checks pass. The lookup remains test-only until complete
query adjustment is implemented, and does not change the live encoder.

Complete reference query, raw adjustment and two-pass request resolution are now
implemented and exported. All 32 parameter tests pass in four configurations;
262 query profiles, 223 adjustments and 450 scalar-state resolutions match the
frozen oracle, with six explicitly classified reserved/overflow rejections.
The independent 46-row review probe passes safe/all replay, including its five
intentional Rust-domain errors. Both no_std checks, strict Clippy and all four
parameter usage examples pass. Numeric encoder execution remains a later
milestone; the new allocation-free API inspects reference profiles only.

Eleven source-anchored official scalar integration tests now exercise that
public API, including the original adjustment and dictionary-size equalities.
All eleven pass in four safe/feature/std configurations, with strict focused
Clippy and formatting. The adapters label detached-request and frame-default
subsets precisely; live lifecycle tests and inventory updates remain separate.

Parameter-only and full encoder resets now implement their documented phase
contracts, clear every selected/active dictionary handle and both seed caches,
and preserve exact codec allocator owners without allocation or cloning. Five
new tests cover phase legality, ownership and allocation-failure recovery;
safe/all streaming suites each pass 26 tests. Focused feature/no_std checks,
strict Clippy, two narrow strict-provenance Miri tests and 36 independent C frame
decodes pass. All four streaming usage examples pass safe/all. Public official
lifecycle adaptations remain the next independent migration checkpoint.

The scalar platform checkpoint passes 86 strict-provenance Miri executions:
74 across i686 safe/all applicable unit and public-integration suites, plus
twelve selected big-endian s390x executions. Six host-64-only normal tests are
explicitly excluded on i686. Safe/all thumbv6m no_std compilation also passes.
The isolated snapshot remained unchanged; this qualifies interpreted Rust and
compile-only targets, not native C/platform release behavior.

Six official lifecycle adaptations now pass through public APIs in four
configurations, preserving the original dictionary/source dimensions and size
inequality with a declared RDG corpus. Existing 288 safe/all default fingerprint
comparisons remain unchanged. The pinned CLI exactly decodes sixty configured,
session-reset, parameter-reset, prefix and full-reset frames. Coverage explicitly
distinguishes selected named policies from the Greedy defaults restored by reset.
The source/case inventory update and complete feature matrix remain open.

The corresponding inventory checkpoint updates 25 anchors with exact supported
subsets. Six pure scalar groups are now ported; whole source files remain
partial. Offline and pinned-source audits pass, while the explicitly separate
complete-migration gate still reports 188 unfinished source units and partial
external corpora. Case counts are 53 ported, 117 partial, 430 blocked and
15 non-applicable. The complete Module 10 feature matrix is the remaining
validation checkpoint.

## Module 11: design and pre-change compatibility inputs

While the immutable Module 10 matrix runs, the next module's ownership and
resource design is recorded before implementation. It separates operational
`MatchSearchOptions` from reference profiles and decomposes dynamic hash/chain
storage, exact-width keys, actual parser effort, retained history, live context
integration and verification. Initial limits account for all seven cached search
vectors. Multi-owner growth preserves logical state on failure while allowing
earlier capacity growth, and every allocation retains its exact allocator owner.
An independent design review clarified both width-dependent Chain seed bounds;
empty one-shot frames explicitly retain zero effective-history behavior.

Default behavior was independently captured at `ba4529c` before any search
implementation change. Twelve deterministic workloads cover multiple blocks,
retention/window boundaries, raw seed thresholds, prepared dictionaries and
short Flush crossings. Each backend produces 36 fresh, 36 reset-reused and four
strategy-cycle frames. Safe/all output fingerprints and metadata agree; pinned C
exactly decodes all 152 complete frames. The source snapshot remained unchanged.

[Baseline evidence](evidence/2026-10-09-match-search-baseline.json) links a compact
archive of original records, source recipes, reproduction programs and logs.
The archive retains SHA-256 and FNV fingerprints without duplicating 20 MiB of
encoded bytes. Future offline fingerprint comparisons remain distinct from the
original complete C byte comparisons. Historical capacity observations include
all codec and selected dictionary storage; a future search-only metric is not
inferred from them. This checkpoint implements no new controls or algorithms.

The independent hash-oracle checkpoint now freezes 2,880 pinned C observations,
2,304 unused-byte pairs with full-width controls, and 64 historical arithmetic
guards. Optimized and ASan/UBSan reproduction agree; source provenance and
immutable checks pass. Isolated corruption/report-overwrite tests and three
independent generic-audit rejection checks pass. The offline inventory now
verifies 324 fixtures in ten collections. This is primitive C evidence only;
the Rust hash implementation and live controls remain separate work items.
See [match-search validation](match-search-validation.md).

The plain search-record foundation validates every option atomically and derives
key dimensions, effective history, policy effort and all-cache requested bytes
without allocation. Seven tests pass in four native feature/std configurations
and on both i686 and big-endian s390x under Miri; no_std, strict Clippy and
formatting pass. Costs use actual target layouts with a 32 MiB requested-payload
ceiling. The records remain private until meaningful consumers are integrated.

The offline default regression is active before owner reshaping: seven tests
pass in all four feature/std configurations, replaying 304 frames against 608
unchanged captured records. Full Rust decodes and fresh/reset byte comparisons
pass. A registered standalone SHA-256 auditor connects the original archive,
reproduction sources, TSVs and current recipe inputs; normal and optimized
Python pass two positive audits and forty expected isolated rejections. The
original independent C evidence remains unchanged.

Exact-width keys and all five pinned unsalted hashes are now implemented in
safe Rust. All 2,880 primary vectors, 2,304 paired controls and 64 historical
guards pass in four configurations. Nine focused strict-provenance Miri
executions pass across host, i686 and s390x, including exact short allocations
and little-endian witnesses. Key widths are checked independently from bucket
counts; no padded missing input or unsafe extraction is introduced.

The Greedy candidate table now supports checked dynamic layouts with exact-owner
reconfiguration and fallible compatible seed transfer. Both access backends
check live initialized bounds before reading or writing. Fourteen leaf tests
pass in six configurations; no_std, strict Clippy, release bounds and six narrow
Miri checks pass. Safe/all default frames still match their original records.
The public encoder continues using default dimensions until context integration.

The bounded candidate chain now supports dynamic head/node shapes and up to
64 examined links. Reconfiguration preserves complete logical state if either
reservation fails, while each allocation keeps its exact owner. Twenty-three
tests pass in seven configurations, five focused Miri checks pass, and safe/all
default frames remain unchanged. Seed compatibility, tiny wrapped rings, stale
links and initialized-length checks are covered before parser integration.
