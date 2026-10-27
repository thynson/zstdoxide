# Development progress

Detailed completed work items are preserved in Git history. `TODO.md` contains
only active module checklists and the remaining roadmap.

## 2026-10-12: wider DoubleFast direct families

168 predeclared constructed inputs and one separately frozen long-literal
refinement now have complete unchanged-C sequences and both final table maps.
All 676 padding-case executions agree across normal/scoped ASan/UBSan builds.
The initial long-literal candidate's zero-match output remains preserved; its
refinement observes the long-literal marker without rewriting the first case.
Normal/optimized offline checks cover all observations, 22 equivalence pairs,
20 sequence witnesses and 32 rejection controls per mode. The fixture registry
contains 715 files in 19 collections. See `docs/reference-double-fast-families.md`.
This completes the direct-oracle data item, not Module 13, public DoubleFast
execution or the complete official test migration.

## 2026-10-12: direct DoubleFast small-log hashes

512 direct C width8 observations now cover logs6..9 and aligned/unaligned
reads of the existing 64 fixed patterns. Normal and scoped sanitizer captures
agree. Rust bucket, collision and full-table comparisons pass in five feature
configurations; the selected foundation and Fast/named baseline tests total 77
passing executions. Clippy, no_std and formatting checks also pass. The new
compiler-free fixture brings the registry to 546 files in 18 collections.
See `docs/reference-double-fast-hashes.md` for complete evidence and scope.
Wider parser families remain open; no production codec behavior changes here.

## 2026-10-12: initial DoubleFast direct oracle

The bounded ZDFST01 helper and pre-execution allocation review are complete.
Fifteen constructed cases match across two padding patterns and normal/scoped
ASan/UBSan builds (60 C case executions). Independent replay checks both full
table maps and wire/parser repeat states. A frozen compiler-free collection
and checker add sixteen files to the registry, now 545 files in 17 collections.
Normal and optimized Python each pass 17 positive and 102 rejection controls.
See `docs/reference-double-fast-validation.md` for observed boundaries, retained
evidence and sanitizer limitations. This completes only the initial oracle
work item; wider families, Rust parsing and public DoubleFast execution remain
open. It does not complete Module 13 or the upstream test migration.

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

Module 10 is now complete within that scalar/lifecycle scope. Its immutable
`0111aef` snapshot passes fifteen full suites, each with 759 tests passed and
52 explicitly ignored, plus fourteen no_std configurations, strict Clippy,
formatting and sixteen usage examples in safe/all builds. Pinned C decodes
sixty complete lifecycle frames; both independent parameter probes and all
934 fixture rows agree with their classified outcomes. All 677 snapshot files
and external oracle inputs remain unchanged. Independent post-verification
checks every log hash and excludes zero-test selector success. The separate
complete-migration gate still reports 188 unfinished source units; numerical
encoder execution and the remaining official migration are not claimed.
See [final parameter validation](compression-parameters-validation.md).

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

Parser probes now use selected key widths and concrete Chain/Lazy visit budgets.
Seven new tests demonstrate exact tails, a wider-key/four-byte match collision,
actual budget-dependent selection, both Lazy probes and preflight failure
atomicity. Thirty-two tests pass in five configurations, public strategy/default
fingerprints remain unchanged, and three narrow Miri checks pass. These remain
private capabilities pending live encoder configuration.

Borrowed source keys now span dictionary, ring history and input at each selected
width, with explicit incomplete tails and checked indexed/absolute coordinates.
Seven new tests pass in six configurations, six existing boundary executions
pass, and eight focused Miri executions pass. Both no_std checks, strict Clippy
and formatting pass. Existing consumers remain unchanged before storage wiring;
no new C execution is attributed to this helper-only checkpoint.

Frame-local search shapes now drive working tables, dictionary seeds, exact-width
source keys and both bounded parser efforts. Short dictionaries cannot revive
old-layout seeds, and a separate capacity sum counts all seven possible search
vectors. Eight new tests pass in seven configurations; safe/all default replay,
eight existing ownership/entropy executions, no_std and strict Clippy pass.
All 22 checks succeed, with 78 passing test executions. The selected history
horizon and public boundary remain the next separately committed consumers.

The selected output-history horizon now controls the real ring reservation,
capped by the frame window, while empty frames allocate no ring and defaults
retain existing bytes/capacities. Seven new tests pass in eight configurations,
including tiny wrapped tails, all-policy distance-196,608 witnesses, independent
dictionary eligibility, exact-owner growth failure/recovery and nonpublication
after output failure. All 23 checks pass with 92 test executions. Public search
configuration is the remaining context integration checkpoint.

Public `MatchSearchOptions` and Encoder search getter/setter/capacity are now
available with operational consumers. Phase-first installation preserves all
handles/owners; session resets keep settings and parameter/full resets restore
both defaults. Five new stream tests join 31 passing tests in eight builds,
totaling 248 executions. Both no_std checks, strict library Clippy and all 18
registered usage examples safe/all pass. Separate public control-effect and C
interoperability campaigns follow before the final module matrix.

All six public search controls now have independent actual-effect witnesses.
Ten tests pass in each of eight configurations, including all 82 observed
allocation-failure ordinals per build and exact-owner recovery. Pinned C
independently decodes 54 complete configured frames across safe/all builds,
comparing 1,208,638 source bytes exactly. All 16 focused commands pass; no
production or stored-default fixture change was needed. Original applicable
search lifecycle assertions, final platform/matrix validation and benchmark
harness/measurement remain before Module 11 completion.


## Module 11 completed: configurable named match search

The supported scope now includes checked search dimensions, exact-width keys,
concrete Chain/Lazy effort, retained-history control and public lifecycle APIs,
while the independent pre-change default frames remain unchanged. Supported
source-backed dictionary/search ordering and reset assertions are ported; the
numeric-level and attachment gaps remain explicit.

The immutable final correctness snapshot passes fifteen full suites, each with
842 tests passed, zero failed and 54 explicitly ignored. All fourteen no_std
library configurations, strict Clippy safe/all, formatting and 36 registered
example executions pass. Pinned C independently decodes 66 complete configured
frames with exact byte comparisons. Fifty-six focused Miri executions across
explicit foundation/leaf/public snapshots and four final thumb compilation/
caller-code-generation checks cover the documented platform boundaries; these
are not native foreign-platform execution.

The frozen timing campaign records fourteen configurations, 27 rows each and
five samples per row, with task compilation/tests/Miri finished before clocks.
All 378 non-timing records agree, and normal/optimized archive replay passes.
The 1,890 samples show workload-specific memory, compressed-size and speed
tradeoffs; all-feature safe/selected median ratios span 0.918–1.099. They do not
establish a universal unsafe speedup. Full evidence, scope qualifications and
reproduction commands are in [validation](match-search-validation.md) and
[measurements](match-search-benchmarks.md).

The Module 11 checklist is cleared; Module 12's reviewed reference-Fast design
and decomposition are retained. The upstream inventory still reports 188
unfinished source units. Neither full official-test migration nor roadmap
completion is claimed.


## Module 12 completed: bounded reference Fast execution

Completed on 2026-10-12 after the pause. The allocator-aware whole-buffer level
and request APIs execute the pinned reference's no-dictionary Fast profiles,
including level 1 and negative levels across the source tiers. The bounded
implementation preserves sampled cross-block history, explicit repeat intent,
negative-level acceleration and raw-literal policy, truthful known-size frame
metadata and transactional raw/RLE/compressed publication. Its entropy output
is independent; higher reference algorithms, dictionaries on these numeric APIs
and live numeric streaming remain outside this milestone. Named defaults retain
their original behavior.

The official negative-level size assertion, inherited source and both C frames,
81 original simple-round-trip selector observations, admitted simple profiles
and original empty/30-byte numeric subsets are preserved and executable with
explicit remaining context/capacity/advanced-branch gaps. Two fresh complete
public interoperability campaigns each checked 316 C decodes of Rust frames and
316 Rust decodes of C frames with exact source comparisons. These successes do
not promote the 188 unfinished upstream source units to complete migration.

The frozen final matrix completed fifteen full suites at 956 passed, zero
failed and 54 explicitly ignored each. Fourteen no_std configurations, strict
Clippy safe/all, formatting, thirteen allocator-helper tests and 42 registered
example executions pass. A separate platform checkpoint passes 24 focused
strict-provenance Miri executions on i686/s390x and four thumb library/caller
compilation checks, with interpretation and compile-only limits explicit.
The pause marker was applied after every matrix command finished; the resume
audit preserved it and verified unchanged inputs rather than rerunning checks.

All fourteen backend configurations were then measured on nine deterministic
cases with five samples each. A short safe/all diagnostic added ninety samples
to the initial 630. Identical executable hashes produced materially different
relative timings, so no repeatable unsafe speedup or causal individual-backend
ranking is claimed. All 144 non-timing frame/allocation records match the frozen
clock-free baseline. Both campaigns, including the unfavorable repeat, remain
in the evidence archives. See [validation](reference-fast-validation.md) and
[measurements](reference-fast-benchmarks.md).

Module 13 preparation freezes nine complete Fast frames and exact qualified
allocator records in four permanent tests, passing twenty executions across
five configurations plus fourteen unchanged named-baseline executions. Its
offline provenance checker passes normal/optimized controls. The independently
reviewed DoubleFast ownership and direct-C arena protocol are also recorded.
The Module 12 completed checklist is cleared; Module 13's remaining work items
are retained. Full official-test migration and the rest of the roadmap remain
open.

## Module 13 completed: bounded reference DoubleFast execution

Completed on 2026-10-12. The numeric one-shot APIs now execute resolved Fast
and DoubleFast profiles, with independent window/table limits and complete
allocator ownership. DoubleFast uses two sampled position tables, an eight-byte
long key, the pinned repeat/update order and persistent cross-block state.
Common framing and entropy publication preserve the existing Fast and named
frames and allocation contracts. Levels 0 and 3 are equivalent, level 2 works
across all source tiers, and the supported level 4 tiers are documented. Numeric
dictionaries/streaming and higher reference algorithms remain outside this scope.

The successor official-test layer retains every original C selector and source
identity. It admits 61 of 81 original selector records (16 unsupported, 4 advanced)
and the original empty/30-byte loops admit 5/24 and 3/19 profiles. Context reuse,
capacity, dictionary, in-place and other advanced obligations remain explicit.
The broader bidirectional campaign completes 159 frozen requests: 113 admitted,
42 unsupported and 4 resource errors. It performs 452 C decodes of Rust frames
and 452 Rust decodes of C frames, reproducing 223,677,280 source bytes. All 26
used C codec units and the helper are instrumented in the ASan/UBSan build;
normal and instrumented C frames agree. No prebuilt C archive is used there.

The final matrix passes fifteen full suites at 1,021 tests each, zero failed and
54 explicitly ignored per suite: 15,315 passing executions. All fourteen no_std
configurations, strict Clippy safe/all, formatting, thirteen allocator-helper
tests and 44 registered example executions pass. A focused platform checkpoint
passes 32 strict-provenance Miri executions on i686/s390x and four thumbv6m
library/caller compilation checks. Interpretation and compile-only results are
qualified separately from native execution.

All fourteen backend configurations were measured on sixteen frozen cases,
followed by a safe/all repeat using identical executable hashes: 1,280 samples
in total. All 256 complete frame/allocation records match the clock-free capture.
The two campaigns show no repeatable all-feature speedup, so the safe default
is retained. Individual results, unfavorable samples, exact allocation scope,
CPU-barrier records and host limitations are preserved in
[measurements](reference-double-fast-benchmarks.md).

The [validation report](reference-double-fast-validation.md) and
[closure reconciliation](evidence/2026-10-12-double-fast-closure.json) bind the
final artifact chains and unchanged measured source. The completed checklist
is cleared and the next Greedy module's decomposition is retained. The upstream
inventory still reports 188 unfinished source units; full test migration,
legacy success assertions and the remainder of the roadmap remain open.


## Module 14 completed: bounded reference Greedy execution

Reconciled on 2026-10-13 from the completed 2026-10-12 campaigns. Known-source,
no-dictionary numeric one-shot requests now execute bounded reference Greedy,
selecting chains through window log 14 and rows above it. Limits are W21/H19,
chain C18/S8 and at most 64 row visits; the unused row chain dimension adds no
resource cap. Level 4 works across all source tiers. Existing named/Fast/
DoubleFast complete frames and allocator contracts are preserved.

All 173 direct cases match complete maps and sequences. Ordinary retained state
matches 395 C blocks, including 25 raw and 12 RLE discarded-repeat transitions;
private frame checks cover 274 frames and all 6,184 observed allocation-failure
ordinals. The successor admission retains all 81 selectors, with 62 admitted,
15 unsupported and four advanced; the original loops admit 6/24 and 4/19.
Thirteen newly admitted assertion tests pass in safe/all builds, without
promoting any complete upstream source unit.

Public interoperability completes 422 recipes: 372 admitted, 37 unsupported
and 13 resource rejects. Across 4,786 commands, C decodes 1,488 Rust frames and
Rust decodes 1,488 C frames, reproducing 413,994,400 source bytes; 156 malformed
requests reject. Normal and instrumented builds compile all 26 used C units
plus the helper. Upstream sanitizer annotations remain and LeakSanitizer is off.

The immutable final matrix passes 17 suites (safe, fourteen isolated features,
all debug/release), each with 1,098 passes, zero failures and 54 explicit ignores:
18,666 passing executions. Formatting, strict safe/all Clippy, sixteen no_std
configurations, thirteen allocator-helper tests and 22 documentation examples
per safe/all mode pass. Focused platform checks add 44 strict-provenance Miri
executions on interpreted i686/s390x, one native smoke and four thumbv6m
library/caller compilation checks; no foreign-native or memory-fit claim follows.

All sixteen backends pass 512 clock-free cases. The all-backend timing campaign
and safe/all repeat retain 2,880 samples in 576 cases, identical frames and owner
records, and completed task-owned CPU barriers. The all/safe geometric means
across 24 Greedy cases are 1.0480 and 1.0588; the repeat spans 0.8546..1.2181.
One host, fixed order, uncontrolled desktop/thermal activity and unrepeated
individual flags prevent universal or causal speed claims. Safe remains default.
See [measurements](reference-greedy-benchmarks.md) for all unfavorable results.

The [closure reconciliation](evidence/2026-10-13-greedy-closure.json) rechecks
thirteen reports, nine archive identities, 1,217 immutable matrix input files
and eighty input identities in each clock-free/timed/repeat campaign. Existing
checks are reused without claiming a fresh full-suite run. Later additive
benchmark and Module 15 preparation files are not retroactively in that matrix;
the nine Greedy baseline tests retain separate safe/chain/row/all qualification.
The [validation report](reference-greedy-validation.md) distinguishes historical
checkpoints from this completed bounded scope. The completed checklist is cleared
and the Lazy/Lazy2 decomposition remains. The inventory still has 188 unfinished
source units (149 blocked, 34 partial, five planned); complete official migration,
numeric dictionaries/streaming, higher strategies and the roadmap remain open.


## Module 15 completed: bounded reference Lazy and Lazy2

Completed on 2026-10-13. Known-source no-dictionary one-shot numeric requests
now execute bounded depth-one and depth-two reference parsing through shared
chain/row owners, checked speculative coordinates and score arithmetic. Limits
remain W21/H19, chain C18/S8 and at most 64 row visits. Default levels 5/6 work
across source tiers; 7/8 through 256 KiB and 9/10 for 16,385..262,144-byte sources.
Private dispatch retains transactionality, owner release and failure poisoning.

All 645 direct cases match captured trajectories. Ordinary capture covers 510
cases and 1,377 blocks; private checks cover 1,014 frames and 23,998 observed
allocation-failure ordinals per configuration. Public interoperability has 886
recipes and 10,104 commands: 3,244 frames in each direction reconstruct
680,435,552 bytes, with 156 malformed exchanges. The versioned v2 auditor binds
464 new tuples independently and retains all 422 historical tuple checks; the
original v1 gap and unchanged capture are explicitly preserved.

The immutable full matrix passes seventeen suites with 1,136 passing tests each:
19,312 passing executions, zero failures and 918 explicit ignored executions.
Formatting, safe/all strict Clippy, sixteen true no_std configurations, thirteen
allocator-helper tests and 22 documentation examples per mode pass. A retained
correction distinguishes all-feature std builds from explicitly enabled unsafe
leaves with std disabled. Platform evidence adds 60 selected Miri executions on
interpreted i686/s390x and no-atomics thumbv6m library/caller compilation. No
foreign-native execution, linked firmware or device memory-fit claim follows.

All sixteen backends agree on 1,280 clock-free cases. Timed and separate safe/all
repeat campaigns retain 7,200 samples in 1,440 cases, complete unchanged frames
and allocator records. The all/safe geometric means over 48 Lazy/Lazy2 cases are
1.0944 and 1.0861; repeated case ratios range
0.7157..1.7937. One host, fixed order and uncontrolled
desktop/thermal activity prevent universal or causal unsafe speed claims.

The [closure](evidence/2026-10-13-lazy-closure.json) checks 29 reports, their
archives, 1,291 existing matrix inputs and 85 source entries per benchmark
campaign. Module 16 preparation separately preserves the eighty-case Lazy
baseline, audits 103 direct tree input recipes and 9,336 C operation records,
and adds a bounded binary fixture and independent safe Rust reader. Production
tree storage/parser and public strategy 6 remain future work. The completed
Module 15 checklist is cleared; Module 16's decomposition remains. Official
migration still has 188 unfinished units (149 blocked, 34 partial, five planned).

## Module 16 completed: bounded reference BinaryTreeLazy2

Completed on 2026-10-14. Known-source no-dictionary one-shot numeric requests
now execute bounded depth-two parsing with independently owned paired binary-tree
edges, checked deferred sorting, candidate scoring and frontier publication.
Limits are W21/H19/C19/S8; row mode is not substituted. Existing named profiles,
frame behavior and predecessor outputs/owner regressions remain intact.
Default levels5/6 work across tiers,7–9 through256KiB, and10–12 above16KiB through
256KiB; documented resource errors remain distinct from unsupported strategies.

Ordinary capture retains245sources,531positive blocks and365complete C frames.
Public interoperability independently resolves1,197recipes and13,606commands:
C decodes4,396Rust frames and Rust decodes4,396C frames, reconstructing795,034,136
bytes. Both Python modes reject41forged records; C/Rust entropy bytes need not
match. Platform evidence includes44selected Miri executions and no-atomics
library/caller compilation, without a native foreign/firmware execution claim.

The immutable full matrix passes18suites with1,204tests each:21,672passing and
990explicit ignored executions, zero failures. All17true no_std builds, strict
safe/all Clippy, formatting,13allocator tests and22usage examples per mode pass.
The original user-requested interruption is preserved; only12fully completed
original suites and6fully completed continuation suites contribute to totals.

All17backends agree on408clock-free cases. Two timed campaigns retain2,280samples
across456runs with identical full frames and exact owners. All/safe geometric
means over24cases are0.7625and1.2238; the direction reverses, so these results do
not establish a stable unsafe throughput benefit. Single-host/fixed-order and
uncontrolled desktop/clock/thermal effects limit interpretation.

The closure evidence checks38reports/archives,1,370matrix input files and95source
identities per benchmark campaign. Historical ordinary-frame storage is verified
by lossless reconstruction of all4,117members rather than unavailable original
gzip metadata. Module16's completed checklist is cleared. Module17 retains its
independent Optimal decomposition; public strategy7 remains unsupported. The
upstream migration gate still reports188unfinished units, so this bounded
milestone is not a claim of complete library/CLI parity.

## Module 17 completed: bounded reference Optimal

Completed on 2026-10-16. Public numeric strategy 7 now implements qualified
complete fresh no-dictionary optLevel=0 frames: source 64..16384 fitting the
window, W10..16/H6..12/C6..12/S1..6, minimum 3..7 and target<=4095. Default levels
11/12 work for source 64..2048; source-aware resolution and existing selector
behavior are preserved. Outside qualified strategy-7 bounds requests return
ResourceLimit; Ultra/Ultra2 remain unsupported. Private retained search/path and
publication are independently qualified but public retained Optimal, dictionaries,
window expiry and LDM are outside this milestone.

Ownership is explicit: an allocation-free plan validates nine layouts before
allocator cloning; independently supplied owners hold frequencies, paths,
matches, search structures and token scratch. Exact borrowed/non-cloning owner,
allocation failure, reset and poison contracts are tested. The shared entropy
path accepts qualified three-byte Optimal matches without weakening earlier
parsers. Default code remains safe Rust; nineteen independently selectable
unsafe features retain matching public contracts and no_std plus alloc support.

The public oracle runs 1269 independently resolved recipes and 14350 commands.
Pinned normal/instrumented C decodes 4636 Rust frames and Rust decodes 4636 C frames,
reconstructing 797537304 bytes. Both Python modes reject 46 forged captures. All
26 codec translation units in the instrumented oracle are covered. Full C/Rust
compressed-byte identity is not required. Platform evidence passes 54 selected
Miri executions across 32-bit/big-endian targets and 10 no-atomics library/concrete
caller builds; this is not native firmware execution.

The unchanged final matrix passes 22 complete suites with 1340 tests and 55 explicit
ignores each: 29480 passing and 1210 ignored executions. Exact case names/outcomes
match across every suite. Another 1729 focused feature executions, 21 host no_std
builds, strict safe/all Clippy, formatting, allocator checks and 44 usage checks
pass. Ten standard offline checks retain 484 unchanged inputs. Ignored upstream
cases and the 188 unfinished source units remain open migration obligations.

All 21 backends produce identical complete frames and allocator surveys across
504 clock-free cases. Two non-overlapping timed campaigns retain 2760 samples for
552 case/backend runs, including an independent safe/all repeat. Complete raw
samples, all 44 executables, source identities and malformed timing controls are
retained. All/safe geometric means of median ns/byte are 0.9493 and 0.9393;
individual cases can still be slower. See the benchmark report for fixed-order
desktop limitations; this is not evidence of a universal unsafe throughput benefit.

The closure report verifies 22 selected archive/report pairs and unchanged final
tested/measured source inventories. Failed predecessor evidence remains retained.
Module 17's completed checklist is cleared; Module 18's Ultra ownership design and
independent work decomposition are ready. This bounded milestone does not claim
complete upstream library, CLI, corpus or format-feature parity.

Closure evidence: [final reconciliation](evidence/2026-10-16-optimal-closure.json).

## Module 18 partial checkpoint: Ultra prices, frequencies and scripted paths (2026-10-16)

Checked fractional price policy, immutable frequency ownership and private scripted
Ultra paths are implemented. Existing Optimal constructors, fixtures and public
behavior are preserved. Public Ultra (strategy 8) and Ultra2 (strategy 9) remain
unsupported; Module 18 is not complete.

The scalar oracle covers 2470 rows and exhaustive independent raw-statistic
boundaries. The frequency oracle covers 298 lifecycle requests, 4065 transitions
and 2848 queries, including exact fractional limits. Complete frequency checkpoint
testing passes 21 selected configurations and full safe/all suites of 1363 passing
and 55 ignored tests each. Selected owner checks pass 28 foreign Miri executions
and no-atomics library/concrete caller builds. The focused frequency benchmark
retains 540 samples with all/safe aggregate ratios near one; it does not establish
a stable unsafe speedup or complete-frame throughput.

The complete scripted path observer retains 58 requests, 65,672 logical events,
72 signed alternatives and 143 sequences, with normal/instrumented and two-padding
equality. Both Python modes reject 44 capture forgeries and 24 fixture forgeries
each. The independent borrowed Rust reader verifies signed operands and exact
next-cell/repeat/frontier publication. Private Ultra path composition matches
every captured state. Four new tests qualify exact owners, bounds/policy rejection
and observer-failure recovery; selected parser platform checks pass eight foreign
Miri executions and safe/all thumbv6m library and concrete parsing code generation.

An additive source-driven ordinary model reconciles both passes of all twenty
original complete fresh C cases (129,220 events). A frozen 44-request manifest
preserves all original inputs under both policies and adds two source-only inputs
covering all eight Ultra-specific path branches. These ordinary Ultra predictions
have not yet been observed natively. The native adapter/command review, ordinary
fixture and Rust integration, execution planning, entropy publication, public
admission, complete interoperability and complete-frame measurements remain open.

Detailed evidence and qualifications are in `reference-ultra-validation.md`,
`reference-ultra-frequency-benchmark.md` and `reference-ultra-fresh-model.md`.
The module checklist is retained in `TODO.md`; completed commits preserve the
detailed work archive and all official migration obligations remain open.

The final private path checkpoint passes 69 commands against unchanged Rust/Cargo
inputs: 21 selected backend configurations, true no_std in each, full safe/all
suites of 1373 passing and 55 ignored tests each with equal individual outcomes,
release parser regressions, strict all-target Clippy safe/all and formatting.
The complete record contains 3397 passing and 110 ignored test executions in
`evidence/2026-10-16-ultra-path-matrix.tar.gz`. These counts supersede the earlier
frequency checkpoint for the current source, without changing either checkpoint's
historical evidence or the remaining public Ultra gates.


## Module 18: bounded reference Ultra complete (2026-10-20)

Public strategy 8 now admits independently qualified fresh no-dictionary sources
of 64..16384 bytes fitting the selected window, W10..16, H/C6..12, S1..6,
minimum-match 3..7 and target 0..4095 after source resolution. Default levels
13/14 support 64..2048-byte sources. Fractional pricing, immutable frequency
policy, thorough relaxation and match-plus-literal alternatives are coupled.
The original Optimal inputs and 4636 successful encoded responses are unchanged.
Ultra2, retained public blocks, dictionary/prefix, LDM, workers, numeric streaming
and window expiry remain outside this milestone.

The ordinary parser, checked allocation-free plan, exact nine-owner token state
and shared-entropy frame dispatcher are qualified in small implementation steps.
Public geometry rejection precedes owner cloning; unsupported and operational
resource classifications remain distinct. Safe defaults and nineteen separately
selectable unsafe implementations retain the same contracts and no_std + alloc.

Native interoperability covers 1345 recipes and 15132 commands, with 4888 frame
decodes in each direction and 800050968 decoded bytes. All 26 relevant C units
are covered by the instrumented build. Both Python modes reject 56 forged native
captures without child execution. Complete C/Rust compressed-byte identity is
not required. Selected 32-bit/big-endian Miri and no-atomics library/concrete
caller checks pass; these are not native foreign or firmware executions.

The full public regression matrix completes 22 configuration products, each with
1405 passes and 55 explicit ignores (30910 passing and 1210 ignored executions),
and 21 true no_std builds. Individual test names and outcomes agree. Six products
are explicitly reconciled from successful prefixes and resumed integration/doc
suffixes after preserved 1800-second timeouts; sixteen complete uninterrupted.
Scoped historical source inventories have explicit Git-bound supplements rather
than invented contemporaneous entries. Strict Clippy, formatting, allocator
support and 48 standalone usage checks pass. Fresh final focused safe/all Ultra,
Clippy, formatting and no_std checks pass on identical production sources.

All 21 backends produce identical complete frames and allocator surveys over
26 preselected cases. Two timed campaigns retain 2990 samples for 598 case/backend
records and all 23 measured executables, including an independent safe/all repeat.
The all/safe geometric means of median cost per byte are 1.0395 and 0.8782: the
opposing directions do not establish a stable unsafe speedup. The frame report
preserves per-case results and fixed-order desktop limitations.

The final reconciliation verifies selected archive/report identities and current
source equality. Historical failed/interrupted attempts remain evidence. Module
18's completed checklist is cleared and Module 19's Ultra2 lifecycle decomposition
remains. The 55 ignored tests, 188 unfinished upstream source units and all other
official migration obligations stay open; this is not full zstd library/CLI parity.

Details: `reference-ultra-public-qualification.md`,
`reference-ultra-frame-benchmark.md`, `evidence/2026-10-20-ultra-closure.json`.

## Module 19: bounded reference Ultra2 complete (2026-10-20)

Public strategy 9 admits fresh no-dictionary sources of 64..16384 bytes fitting
W10..16, H/C6..12, S1..6, minimum-match 3..7 and target 0..4095 after resolution.
Default levels 16/17 fit 64..2048-byte sources. The discarded first pass retains
learned frequencies and exact nine-owner identities; checked rebasing excludes
old search history, clears token lengths and restores original repeats. Only
the completed second pass reaches shared entropy. Public retained blocks,
dictionary/prefix, window expiry, numeric streaming, LDM and workers remain
outside this milestone.

The independently observed fixture contains 53 cases, 394656 events and 29
resets, preserves 44 old first-pass identities and covers stale ring aliases
and changed final paths. The private lifecycle passes all 22 configuration
products and 21 true no_std builds. Selected 32-bit/big-endian Miri and safe/all
no-atomics concrete code generation pass; these are not foreign native runs.

Public native interoperability covers 1433 recipes: 1292 admitted and 141
resource rejections. All 16008 commands complete with their declared outcomes,
including 156 expected malformed-input exits. Each direction decodes 5168
frames; total decoded bytes are 802569512. All 26 relevant C units receive
sanitizer instrumentation. All 1345 predecessor recipe/source pairs and 4888
successful predecessor Rust responses remain intact. Both Python modes reject
59 native-evidence forgeries without child execution.

Full public regression completes 22 uninterrupted final configuration products,
each with 1435 passes and 55 explicit ignores (31570 passing and 1210 ignored
executions), plus 21 true no_std builds. Independent reconciliation compares all
67 named groups and exact outcomes. The earlier empty-source classification
assertion failure and three cancelled incomplete attempts are preserved and
excluded from success counts. Formatting, strict safe/all Clippy, allocator,
usage, platform and inventory evidence applies to unchanged production sources.
The 188 unfinished upstream source units and 55 ignored cases remain open.

All 21 backends produce equal complete frames and allocator ledgers on the 26
preselected cases. Primary timings and an independent safe/all repeat retain
598 case/backend rows, 2990 samples and all 23 measured executables. Both modes
pass 33 timing controls with zero child attempts. The all/safe geometric means
of median cost per input byte are 0.9985 and 1.0007; they do not establish a
stable unsafe speedup. CPU barriers exclude task-owned competing work, while
fixed order, thermal state and unrelated desktop activity remain limitations.

Final reconciliation verifies all 23 Ultra2 archive/report pairs, 45 predecessor
pairs, current full-regression and measured source inventories and all 90 public
production identities. Historical stages and failed attempts remain explicit.
Module 19's checklist is cleared. Module 20 begins with a reviewed retained
Optimal ownership/scheduling/publication design: independently qualify sources
above the current private 16 KiB cap before extending bounded complete frames
to 32 KiB. This design changes no current production admission.

Details: `reference-ultra2-public-qualification.md`,
`reference-ultra2-frame-benchmark.md`, `evidence/2026-10-20-ultra2-closure.json`,
and the Module 20 design in `data-structures.md`.

## Module 20: retained Optimal public checkpoint (2026-10-20)

Bounded public strategy 7 now supports known-size no-dictionary sources through
32768 bytes fitting the resolved window. Earlier 64..16384 fresh requests retain
their dispatch and frame bytes. Larger sources use three or four balanced blocks,
retaining parser frequencies/search history while publishing repeat and entropy
state only for compressed blocks. Checked plans reject unsupported geometry
before owner cloning, reserve a complete frame bound and retain nine parser
owner identities. Raw/RLE fallback, failed publication, poisoning and reset are
qualified independently. Retained Ultra/Ultra2 and window expiry remain outside
this extension.

The larger-domain independent model, native observations and immutable Rust
fixtures precede implementation. Token and integrated frame contracts pass all
22 storage products and 21 true no_std configurations. Selected foreign Miri and
thumbv6m concrete code-generation checks pass. Actual Rust wire histories are
independently replayed without assuming identical C/Rust entropy choices.

The public native campaign preserves 2866 predecessor response byte strings.
Its 80 additive requests include 61 retained Optimal successes, eight existing
BinaryTreeLazy2 default controls and eleven resource rejections. Native and Rust
decoders reproduce all 552 decoded sources across both directions; all 156
malformed transport checks reject. Independent offline audits and forgery
controls pass in ordinary and optimized Python. Failed attempts remain explicit.

Final public safe/all complete suites each pass 1465 tests and ignore 55, with
identical named outcomes. All 21 true no_std products, strict Clippy, formatting,
13 allocator tests and 54 standalone usage checks pass. Pinned upstream source
and inventory audits pass without claiming completion of the remaining migration.
The other twenty full-suite configuration products still require qualification.

Twenty-six independently decoded complete frames and matching safe/all allocation
ledgers are frozen in a separately qualified, clock-free benchmark snapshot.
No retained Optimal timing result is claimed. Every-backend benchmark preflight,
controller controls, measurements, the full public matrix and final Module 20
reconciliation remain open. See `reference-optimal-retained-frames-validation.md`
and `reference-retained-optimal-frame-benchmark.md` for archive identities and
precise evidence boundaries.


## Module 20: bounded retained Optimal complete (2026-10-21)

The final public entry point is now directly qualified by safe/all foreign Miri
on i686 and s390x and by concrete safe/all thumbv6m core/alloc code generation.
Native-to-final source reconciliation permits only the documented Rustdoc and
lint-reason differences; it does not relabel historical source runs.

All 22 full-suite products agree on 1465 passing and 55 ignored tests, across
69 named groups and 70 Cargo result sections per product. This is 32230 passed
and 1210 ignored executions of the same named suite. All 21 unique true no_std
configurations pass. The initial reconciler confused result sections with named
groups; its failed receipt and source remain preserved, and an additive successor
verifies both counts without rerunning or changing the passing tests. Earlier
formatting, strict Clippy, allocator and standalone usage checks remain exactly
source-bound. No production Rust changed during this final qualification.

The promoted complete-frame benchmark passes all 21 clock-free backends and
ordinary/optimized controller controls. The first ineffective mutation is kept
as a failed control; the corrected successor passes all 43 checks. Complete
measurements and an independent safe/all repeat retain 598 case rows, 2990 raw
samples and all 23 measured executables. Ordinary and optimized timed controls
each pass 33 checks, including 31 effective rejects. All frames and allocator
ledgers agree. All/safe geometric mean cost ratios are 1.0228 and 1.0258, or about
2.3% and 2.6% slower for these equally weighted workloads. Task-owned CPU barriers
are explicit; fixed order, thermal state and unrelated desktop activity limit
broader performance conclusions. Safe remains the default.

Final reconciliation verifies all 21 Module 20 archive/report pairs, 21 earlier
retained pairs and 68 Ultra/Ultra2 predecessor pairs, current full-regression and
measured source inventories, and 92 production/compiler/allocator/adapter inputs.
Module 20's completed checklist is cleared. Its scope remains bounded known-size
no-dictionary strategy 7 through 32768 bytes within one window; all old fresh
requests and frames are preserved. Retained Ultra/Ultra2, dictionaries/prefixes,
window expiry, numeric streaming, LDM and workers are separate. The 55 ignored
cases and 188 unfinished upstream source units remain open.

Module 21's retained Ultra ownership and work decomposition precede implementation.
Its additive model, 42 source-only recipes, pure native preflight and complete
normal/sanitized native state capture are qualified. Ordinary/optimized native
controls and all policy-0 predecessor identities pass. Immutable fixture reading
and production retained Ultra admission remain subsequent gates.

Details: `reference-retained-optimal-public-qualification.md`,
`reference-retained-optimal-frame-benchmark.md`,
`evidence/2026-10-21-optimal-retained-closure.json`, and
`reference-ultra-retained-model.md`.


## Module 21: bounded retained Ultra complete (2026-10-22)

Resolved strategy 8 now supports known sources through 32768 bytes within one
window under the documented resource caps. Fresh requests through 16384 bytes
keep their original dispatch and frame bytes; larger requests use balanced
three/four-block schedules. Policy-2 search and learned prices survive raw/RLE
publication, while only compressed publication commits repeats and wire entropy.
The implementation remains safe by default and compatible with no_std plus alloc.

Private native-state and Rust-wire qualification precede public admission.
The final 1593-case public campaign has 1423 successes and 170 resource-limit
responses. Exactly the declared `retained-resource-ultra` predecessor changes
to success; the other 3024 safe/all response byte strings remain unchanged.
All 3964 capture commands pass, including 496 complete bidirectional decodes and
156 malformed calls. Independent ordinary/optimized audits each pass 21 negative
and two positive controls. The focused controller's empty-doctest mistake is
preserved; its successor inherits 12 successful commands and executes two
remaining checks without claiming inherited commands were rerun.

All 22 full products have 1505 passed and 55 ignored tests,
with identical named outcomes and 72 Cargo result sections.
All 21 true no_std configurations pass. Strict Clippy, formatting, standalone
usage, allocator and upstream inventory checks bind the same production source.
Final public safe/all Miri passes on i686 and s390x; safe/all thumbv6m core/alloc
checks and concrete LLVM IR/ARM objects are retained. These are foreign
interpretation/code generation, not native foreign execution.

The 26 preselected complete-frame benchmark cases pass all 21 clock-free
backends, with identical frames and allocation ledgers. Ordinary/optimized
controller checks each pass 41 negative and two positive controls. Primary and
independently rebuilt safe/all measurements retain 598 case rows, 2990 samples
and all 23 measured executables. Timed controllers each reject 31 forgeries and
accept two positives. Task-owned CPU barriers precede clocks; unrelated desktop
activity, fixed order and thermal history limit broader performance conclusions.
All/safe geometric mean cost ratios are 0.9969 and 1.0104.
No universal unsafe speedup is claimed; safe remains the default.

The final reconciliation verifies 19 module archive/report pairs and 110
predecessor pairs, plus exact qualified source inventories and 92 production,
compiler, allocator and adapter inputs. The completed checklist is cleared;
Module 22's retained Ultra2 first-block bootstrap design and decomposition are
recorded before implementation. Retained Ultra2, dictionaries/prefixes, window
expiry, numeric streaming, LDM and workers remain separate work. The 55 ignored
test obligations and 188 unfinished upstream migration units remain open.

Details: `reference-ultra-retained-public-qualification.md`,
`reference-ultra-retained-frame-benchmark.md`, and
`evidence/2026-10-22-ultra-retained-closure.json`.

## Module 22: bounded retained Ultra2 complete (2026-10-23)

Resolved strategy 9 supports known sources through 32768 bytes within one
window and the documented resource caps. Fresh requests through 16384 bytes
preserve their original dispatch and frame bytes; larger requests use balanced
three/four-block schedules. Only the first block performs discarded learning,
checked rebase and reparsing; later blocks use one retained Ultra-priced pass.
Default builds remain safe Rust and codec modules support no_std plus alloc.

The final 1672-case public campaign has 1486 successes and 186 resource-limit
responses. Exactly predecessor indices 1512 and 1592 change to success; the other
3182 safe/all response byte strings remain identical. All 4132 capture commands
pass, including 504 bidirectional decodes and 156 malformed requests. Independent
normal/optimized native audits each pass 26 negative and two positive controls.
Final public safe/all Miri on i686 and s390x and concrete safe/all thumbv6m
LLVM IR/ARM objects are retained. These are foreign interpretation and code
generation, not native foreign execution. A final Rustdoc-only change is bound
explicitly to the historical executable source, without relabeling old runs.

All 22 full regression products report 1547 passed and 55 ignored tests; all
21 true no_std products pass. Independent reconciliation checks all 43 commands,
74 result sections and 73 named groups, including whitespace-bearing doctests.
Formatting, strict Clippy, usage, allocator and upstream inventory gates pass.
Disk-exhausted compilation, application interruption and recovery-controller
errors remain recorded; no incomplete command is counted as passed.

The 26 complete-frame cases agree across 21 clock-free backends, including
exact frames and allocation ledgers. Normal/optimized controls each reject
41 forgeries and accept two positives. Actual primary and independently rebuilt
safe/all measurements retain 598 case rows, 2990 samples and all 23 executables.
Timed controls each reject 31 forgeries and accept two positives. All/safe
geometric mean cost ratios are 0.9913 primary and 1.3041 repeat. This variation
does not establish a stable unsafe speedup. Fixed order, thermal/compiler state
and unrelated desktop activity limit performance interpretation; safe remains
the default.

Final reconciliation verifies 24 module archive/report pairs, 129 predecessor
pairs and 95 production/compiler/allocator/adapter inputs. The completed Module
22 checklist is cleared; Module 23 retains direct caller-owned one-shot output
analysis and decomposition before implementation. Dictionaries/prefixes, window
expiry, numeric streaming, LDM and workers remain separate encoder work. The
55 ignored obligations and 188 unfinished upstream migration units remain open.

Details: `reference-ultra2-retained-public-qualification.md`,
`reference-ultra2-retained-full-regression.md`,
`reference-ultra2-retained-measurements.md`, and
`evidence/2026-10-23-ultra2-retained-closure.json`.

## Module 23: bounded caller-owned one-shot decompression output complete (2026-10-26)

The scoped API and ownership contract are integrated with default-safe Rust and
no_std plus alloc. Exact destination/options bounds, malformed inputs, allocator
failures and independent dictionary/slice storage configurations are qualified
where applicable. Native helpers remain external test oracles pinned to v1.5.7.
Foreign Miri and concrete thumbv6m core/alloc code generation are interpretation
and code-generation evidence, not native foreign hardware execution.

Complete regression reports 1,563 passed and 56 explicitly ignored
tests across 77 result groups per configuration. Safe, all and twenty
isolated unsafe backends have qualified regression/no_std/Clippy; formatting and
release gates are retained in the validation report. External corpus runs have
separate explicit receipts. Source snapshots and unsuccessful preparatory attempts
remain distinguishable from final successful runs.

The frozen benchmark compares all 22 primary backends and independently rebuilt
safe/all repeats with equal output and allocation qualification. All/safe
geometric mean latency ratios are: primary complete input 0.9698; repeat complete input 0.9763. Host timing variation,
fixed order and unrelated desktop work prevent a universal unsafe-speedup claim.

The final receipt rehashes this module's archives and links the predecessor
closure while binding integrated production to the final checks and measured
executables. The completed checklist is cleared; Module 24's design and
decomposition remain recorded before its implementation is promoted. Complete
upstream migration, unsupported C workspace/ABI behavior and remaining roadmap
contracts stay open.

Details: `caller-output-validation.md`, `caller-output-benchmarks.md`, and
`evidence/2026-10-26-caller-output-closure.json`.

## Module 24: bounded stable borrowed incremental output complete (2026-10-26)

The scoped API and ownership contract are integrated with default-safe Rust and
no_std plus alloc. Exact destination/options bounds, malformed inputs, allocator
failures and independent dictionary/slice storage configurations are qualified
where applicable. Native helpers remain external test oracles pinned to v1.5.7.
Foreign Miri and concrete thumbv6m core/alloc code generation are interpretation
and code-generation evidence, not native foreign hardware execution.

Complete regression reports 1,580 passed and 57 explicitly ignored
tests across 80 result groups per configuration. Safe, all and twenty
isolated unsafe backends have qualified regression/no_std/Clippy; formatting and
release gates are retained in the validation report. External corpus runs have
separate explicit receipts. Source snapshots and unsuccessful preparatory attempts
remain distinguishable from final successful runs.

The frozen benchmark compares all 22 primary backends and independently rebuilt
safe/all repeats with equal output and allocation qualification. All/safe
geometric mean latency ratios are: primary whole 1.0298; primary fragmented 1.0916; repeat whole 0.9215; repeat fragmented 0.9729. Host timing variation,
fixed order and unrelated desktop work prevent a universal unsafe-speedup claim.

The final receipt rehashes this module's archives and links the predecessor
closure while binding integrated production to the final checks and measured
executables. The completed checklist is cleared; Module 25's design and
decomposition remain recorded before its implementation is promoted. Complete
upstream migration, unsupported C workspace/ABI behavior and remaining roadmap
contracts stay open.

Details: `stable-output-validation.md`, `stable-output-benchmarks.md`, and
`evidence/2026-10-26-stable-output-closure.json`.

## Module 25: bounded dictionary-backed caller-owned one-shot output complete (2026-10-26)

The scoped API and ownership contract are integrated with default-safe Rust and
no_std plus alloc. Exact destination/options bounds, malformed inputs, allocator
failures and independent dictionary/slice storage configurations are qualified
where applicable. Native helpers remain external test oracles pinned to v1.5.7.
Foreign Miri and concrete thumbv6m core/alloc code generation are interpretation
and code-generation evidence, not native foreign hardware execution.

Complete regression reports 1,598 passed and 59 explicitly ignored
tests across 84 result groups per configuration. Safe, all and twenty
isolated unsafe backends have qualified regression/no_std/Clippy; formatting and
release gates are retained in the validation report. External corpus runs have
separate explicit receipts. Source snapshots and unsuccessful preparatory attempts
remain distinguishable from final successful runs.

The frozen benchmark compares all 22 primary backends and independently rebuilt
safe/all repeats with equal output and allocation qualification. All/safe
geometric mean latency ratios are: primary complete input 0.9707; repeat complete input 0.9794. Host timing variation,
fixed order and unrelated desktop work prevent a universal unsafe-speedup claim.

The final receipt rehashes this module's archives and links the predecessor
closure while binding integrated production to the final checks and measured
executables. The completed checklist is cleared; Module 26's design and
decomposition remain recorded before its implementation is promoted. Complete
upstream migration, unsupported C workspace/ABI behavior and remaining roadmap
contracts stay open.

Details: `dictionary-output-validation.md`, `dictionary-output-benchmarks.md`, and
`evidence/2026-10-26-dictionary-output-closure.json`.

## Module 26: bounded persistent dictionaries in stable borrowed output complete (2026-10-26)

The scoped API and ownership contract are integrated with default-safe Rust and
no_std plus alloc. Exact destination/options bounds, malformed inputs, allocator
failures and independent dictionary/slice storage configurations are qualified
where applicable. Native helpers remain external test oracles pinned to v1.5.7.
Foreign Miri and concrete thumbv6m core/alloc code generation are interpretation
and code-generation evidence, not native foreign hardware execution.

Complete regression reports 1,612 passed and 60 explicitly ignored
tests across 88 result groups per configuration. Safe, all and twenty
isolated unsafe backends have qualified regression/no_std/Clippy; formatting and
release gates are retained in the validation report. External corpus runs have
separate explicit receipts. Source snapshots and unsuccessful preparatory attempts
remain distinguishable from final successful runs.

The frozen benchmark compares all 22 primary backends and independently rebuilt
safe/all repeats with equal output and allocation qualification. All/safe
geometric mean latency ratios are: primary whole 0.9964; primary fragmented 0.9973; repeat whole 0.9959; repeat fragmented 1.0067. Host timing variation,
fixed order and unrelated desktop work prevent a universal unsafe-speedup claim.

The final receipt rehashes this module's archives and links the predecessor
closure while binding integrated production to the final checks and measured
executables. The completed checklist is cleared; Module 27's design and
decomposition remain recorded before its implementation is promoted. Complete
upstream migration, unsupported C workspace/ABI behavior and remaining roadmap
contracts stay open.

Details: `stable-dictionary-output-validation.md`, `stable-dictionary-output-benchmarks.md`, and
`evidence/2026-10-26-stable-dictionary-output-closure.json`.

## Module 27: bounded single-use prefixes in stable borrowed output complete (2026-10-27)

The scoped API and ownership contract are integrated with default-safe Rust and
no_std plus alloc. Exact destination/options bounds, malformed inputs, allocator
failures and independent dictionary/slice storage configurations are qualified
where applicable. Native helpers remain external test oracles pinned to v1.5.7.
Foreign Miri and concrete thumbv6m core/alloc code generation are interpretation
and code-generation evidence, not native foreign hardware execution.

Complete regression reports 1,628 passed and 61 explicitly ignored
tests across 91 result groups per configuration. Safe, all and twenty
isolated unsafe backends have qualified regression/no_std/Clippy; formatting and
release gates are retained in the validation report. External corpus runs have
separate explicit receipts. Source snapshots and unsuccessful preparatory attempts
remain distinguishable from final successful runs.

The frozen benchmark compares all 22 primary backends and independently rebuilt
safe/all repeats with equal output and allocation qualification. All/safe
geometric mean latency ratios are: primary whole 0.9807; primary fragmented 0.9877; repeat whole 0.9846; repeat fragmented 1.0045. Host timing variation,
fixed order and unrelated desktop work prevent a universal unsafe-speedup claim.

The final receipt rehashes this module's archives and links the predecessor
closure while binding integrated production to the final checks and measured
executables. The completed checklist is cleared; Module 28's design and
decomposition remain recorded before its implementation is promoted. Complete
upstream migration, unsupported C workspace/ABI behavior and remaining roadmap
contracts stay open.

Details: `stable-prefix-output-validation.md`, `stable-prefix-output-benchmarks.md`, and
`evidence/2026-10-26-stable-prefix-output-closure.json`.

## Module 28: bounded dictionary-free in-place decompression complete (2026-10-27)

One exclusive caller buffer holds encoded suffix and decoded prefix. Checked
raw movement permits actual overlap; compressed payload and output history use
disjoint borrows. Dynamic unread-input fences, exact output/window options,
fallible scratch and unchanged success suffix are qualified. The independent
unsafe-in-place-buffer feature shares the safe public contract. Codec modules
remain no_std plus alloc.

All 72 final commands pass: safe/all and 21 isolated feature selections each have
complete regression, true no_std and strict Clippy, plus safe/all full release
and formatting. Every full/release suite reports 1,661 passed / 63 ignored across
94 groups. External ignored native adapters are explicitly run separately.
Normal/sanitized native layouts and the original 5 MiB single/two-frame assertions
use the pinned oracle, including captured preceding original fuzzer history.
Foreign Miri and ARM objects are interpretation/code generation evidence.

All 23 retained primary benchmark backends and independent safe/all rebuilds
have identical allocation/output qualification. Decode, direct, restoration and
combined modes retain equal per-operation clocks and separate actual CPU barriers.
All/safe nonempty median-latency geometric mean ratios: primary in-place-decode 0.8041; primary in-place-with-restore 0.8076; repeat in-place-decode 1.0056; repeat in-place-with-restore 0.9985.
These fixed-order host measurements establish no universal unsafe speedup.

The final receipt rehashes all stage archives, matches final production and test
identities, and links the preceding milestone. The completed checklist is cleared;
Module 29's dictionary ownership design and decomposition remain before promotion.
Persistent dictionary/prefix in-place, incremental overlap, workspace contracts,
legacy input and complete upstream parity remain separate work.

Details: `in-place-output-validation.md`, `in-place-output-benchmarks.md`, and
`evidence/2026-10-27-in-place-output-closure.json`.

## Module 29: persistent dictionaries in in-place output complete (2026-10-27)

Persistent raw/prepared dictionaries remain immutably borrowed while one exclusive
caller buffer holds encoded suffix and decoded prefix. Frame selection validates
IDs, resets repeat offsets and applies immutable entropy fallback without cloned
handles, copied dictionary history or output allocation. Dictionary and scratch
allocators remain independent; fallible scratch is released on each failure.

All 72 final commands pass, including safe/all and 21 isolated complete suites,
true no_std and strict Clippy, safe/all release and formatting. Every full/release
suite reports 1,676 passed / 64 ignored across 97 groups. Normal/sanitized native
helpers qualify 1,836 shared-buffer layouts; explicit empty-input/window-expiry
and unused-suffix differences are retained. All 12,063 pinned dictionary corpus
files execute in safe/all release with zero skips. Prefix selections remain
outside this in-place supplement. Four foreign Miri and four core+alloc/actual
ARM artifact checks pass on the final test source.

All 23 primary and two independent-repeat backends have matching output and
allocation qualification, with separately timed restoration and combined modes.
All/safe nonempty median-latency geometric mean ratios: primary in-place-decode 0.9969; primary in-place-with-restore 0.9597; repeat in-place-decode 1.0237; repeat in-place-with-restore 1.0530.
Fixed-order host measurements establish no universal unsafe speedup.

The closure rehashes focused, matrix and measurement archives and matches final
integrated production/tests. Rejected helper/test attempts remain historical;
no unexecuted check or ignored adapter is credited as passing. The completed
checklist is cleared; Module 30 retains its structural-query decomposition.
Prefix in-place lifecycle, incremental overlap, workspaces and complete upstream
library/CLI parity remain open. See `dictionary-in-place-validation.md`,
`dictionary-in-place-benchmarks.md` and
`evidence/2026-10-27-dictionary-in-place-closure.json`.

## Module 30: checked structural frame queries complete (2026-10-27)

Copied first-frame metadata, encoded extent, exact/unknown concatenated size,
checked decompression bounds and borrowed skippable payloads share one structural
traversal. Complete concatenations validate every tail, even after unknown size;
u64 sizes have no reserved sentinels and arithmetic overflow is explicit. The
query component builds with core only; the complete codec remains no_std + alloc.
Structural success does not prove entropy, checksum or advertised-size truth.

All 72 final commands pass: safe/all and 21 isolated full/no_std/Clippy selections,
safe/all release and formatting. Each full/release suite has 1,690 passes and
67 ignored across 101 groups. External adapters run explicitly in separate gates.
Normal/sanitized native helpers query 10,818 inputs; 550 early-unknown, window-cap
and numeric-sentinel/overflow differences are retained. Exact preceding original
fuzzer histories back single/multi-frame assertions. Original group 948 retains
its partial status: its full-capacity query yields an error sentinel, so its
unsigned comparison does not establish a meaningful upper bound. Useful exact-
frame-length checks are explicitly supplemental. Foreign width/endian Miri and
actual core-only ARM artifacts include the small-input above-4-GiB-bound case.

Twenty-three primary backends and independent safe/all repeats have identical
clock-free query results. Four distinct query modes retain equal batch-loop overhead.
All/safe nonempty-input median-latency geometric means: primary first-frame 0.9940; primary total-size 0.9852; primary total-bound 0.9843; primary in-place-margin 0.9763; repeat first-frame 1.0263; repeat total-size 1.0853; repeat total-bound 1.0184; repeat in-place-margin 1.0258.
These fixed-order host measurements establish no universal unsafe speedup.

Closure rehashes every stage archive and matches integrated production/tests.
The completed checklist is cleared; Module 31 retains its utilities design.
Compression-bound/write-skippable utilities, reusable prefix lifecycle, workspaces
and full upstream library/CLI parity remain open. See `frame-query-validation.md`,
`frame-query-benchmarks.md` and `evidence/2026-10-27-frame-query-closure.json`.
