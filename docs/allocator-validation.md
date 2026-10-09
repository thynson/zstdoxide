# Allocator and platform validation

Module 9 is in progress. This report separates the compiler checkpoint, custom
allocation migration, and platform execution. None of these checkpoints claims
complete upstream API or test parity. The active work items are in `TODO.md`.

## Compiler checkpoint: 2026-10-09

The implementation pin is `nightly-2026-10-08`, reporting Rust
`1.101.0-nightly`, commit `1d81eb4ad9cd207e3e638bd32b17ec4fce8412a6`, LLVM
23.1.3, on `aarch64-apple-darwin`. Installation included rustfmt, Clippy,
rust-src, and Miri. `Cargo.toml` declares a conservative `rust-version` floor
of `1.101`; the supported compiler for this work is the exact dated pin.
No stable 1.101 release is claimed as tested. An actual attempt to install the
official Rust 1.100.0 distribution returned HTTP 404 for its checksum, leaving
that compiler unverified.

The standalone [standard-API probe](evidence/2026-10-09-allocator-api-probe.rs)
compiled successfully with `#![no_std]`, `#![forbid(unsafe_code)]`, and no feature
gates. It exercises owned and borrowed standard allocators, allocator-bearing
vectors, fallible reservation, and direct return of the owning vector:

```sh
rustc --crate-type lib --crate-name allocator_api_probe --edition 2024 \
  docs/evidence/2026-10-09-allocator-api-probe.rs --out-dir /tmp/zstdoxide-allocator-api-checkpoint
```

Before any generic codec migration, the unchanged Module 8 production source
and compiled test/benchmark inputs were fingerprinted before and after seven
commands using `CARGO_TARGET_DIR=target/module9-baseline`. Both fingerprints
remained identical. Default and all-feature suites each passed 534 tests with
45 explicitly ignored tests. Formatting and actual safe/all-backend no_std
checks passed. Strict safe/all-target and all-feature/all-target Clippy each
failed on new `manual_is_multiple_of` and `chunks_exact_to_as_chunks` lints.
These failures are retained as compiler compatibility evidence, not relabeled
successful checks. No ignored external-oracle or corpus campaign was rerun in
this unchanged-source checkpoint.

The [original results](evidence/2026-10-09-allocator-compiler-baseline-results.json)
preserve all commands, exit codes, counts, durations, and log hashes. Lossless
before/after source manifests are archived alongside them. The seven commands
took 90.41 seconds in total; this includes compilation and concurrent work and
is not a codec performance measurement.

Compatibility corrections use `is_multiple_of` for the positive Huffman copy
count and `as_chunks`/`as_chunks_mut` for constant-size slices. They preserve
iteration order and explicit remainder handling. Verification of the corrected
source is recorded separately from the unchanged-source baseline.

The [corrected-source results](evidence/2026-10-09-allocator-compatibility-results.json)
passed all nine final commands in 84.84 seconds: both complete suites again
passed 534 tests with 45 ignored, focused XXH64 tests passed 7 cases, all-feature
Huffman codebook tests passed 19 cases, and formatting, both true no_std checks,
and both strict all-target Clippy checks passed. Production and test/benchmark
fingerprints stayed unchanged throughout those commands. The
[correction index](evidence/2026-10-09-allocator-compiler-corrections.json)
records the eight changed files, initial lint iterations, and final evidence.
No allocator-bearing codec owner had been introduced at this checkpoint.

The Miri filter guard now honors the repository's dated toolchain instead of a
floating `+nightly`. Its [focused check](evidence/2026-10-09-allocator-compiler-miri-guard.json)
passed one real all-feature history test and deliberately rejected an absent
selector with exit code 1. This verifies the guard and selected test only, not
the full Miri workflow. Benchmark shell syntax, guard Python syntax, and the
standalone API probe's rustfmt check also passed.

Module 8 validation and benchmark records remain measurements of their original
Rust 1.89 compiler, source snapshots, and workloads. They are not retroactively
attributed to this compiler pin. Updating the CI workflow describes future CI
execution; it does not establish that the current Linux workflow has run.

## Allocation and platform checkpoints

The separate `zstdoxide-test-allocator` development dependency provides a
bounded, preallocated ledger, positive-size failure ordinals, distinct clone
identities, and a non-Clone owner usable by value or reference. Final
[helper evidence](evidence/2026-10-09-allocator-helper.json) records 13 passing
tests in each of debug, release, and strict-provenance Miri, plus clean strict
all-target Clippy and formatting. Root test-target compilation and safe no_std
compilation also passed after development-dependency registration.

Review found and fixed macOS std Mutex's lazy first-lock allocation: constructor
setup initializes and unlocks the mutex before an allocator owner can exist.
The selected stdlib's subsequent lock/unlock/destruction preconditions, every
unsafe forwarding call, failed-resize preservation, and owner identity were
reviewed again after that fix. The helper's unsafe implementation is outside
the core crate; default codec code still forbids unsafe operations.

Payload counters exclude setup ledger/Arc/native-mutex storage and cannot see
transient overlap internal to Global resizing. Setup retains standard Arc and
native-mutex panic/abort behavior; callbacks use initialized private state and
never dynamically extend bookkeeping. These measurement and platform limits
are documented in the helper README. CI now schedules its tests, strict checks,
and Miri; this edit is not evidence of an executed Linux CI run.

`History<A>` is the first migrated codec owner. Its
[focused evidence](evidence/2026-10-09-allocator-history.json) records seven tests
in each of safe, `unsafe-history`, all-feature, and no-default/unsafe-history
builds, plus seven tests in each safe and unsafe strict-provenance Miri run:
42 passing test executions. Safe/unsafe no_std checks, strict safe/all-feature
all-target Clippy, and the file's formatting check passed. These are focused
leaf checks during concurrent development, not a frozen whole-codec matrix.

The five added tests cover an owned non-Clone allocator and borrowed use,
pointer/allocator-preserving return, every append/repeat/match growth failure,
validation before allocation, absence of hidden requests within reserved
capacity, overlap parity at distances 1 through 16, and swaps between distinct
allocator domains followed by complete release. Default public codec APIs
still use Global; the private generic owner alone does not expose a custom
one-shot decoder.

`RingHistory<A>` retains its allocator without requiring Clone. Its
[focused evidence](evidence/2026-10-09-allocator-ring-history.json) records all
21 tests (14 existing and seven custom) passing in five configurations: safe,
`unsafe-ring-history`, all features, no default features, and no default
features with the ring backend. Safe/all-backend no_std checks, strict
whole-crate Clippy, and owned-file formatting passed. Four exact
strict-provenance Miri tests exercised unsafe-ring growth failure, non-Clone
ownership, distinct-domain swaps, and reuse without allocation.

The new ring tests preserve bytes and all metadata after real initial/growth
allocation failures, verify that same/smaller windows consume no further
requests, and compare fragmented overlap output for every distance in windows
1 through 19. Validation and counter-overflow errors leave both caller output
and history unchanged. This checkpoint preserves the public streaming API's
Global default; allocator-aware streaming integration remains a separate item.

The forward bit writer and normalized-count writer now own `Vec<u8, A>` without
requiring allocator Clone. Their [focused evidence](evidence/2026-10-09-allocator-writers.json)
records six bit-writer tests and 20 entropy tests passing in each of safe,
`unsafe-bitstream`, all-feature, and no-default configurations. One existing
live C entropy oracle remains explicitly ignored. The custom count writer
matched all 512 committed independent C headers. Strict-provenance Miri passed
all six bit-writer tests and five exact count-writer tests, including that full
header corpus. no_std compilation, strict all-target/all-feature Clippy, and
owned-file formatting passed.

Failure injection covers each observed allocation ordinal, writer close/write
state, reservoir and bit position, reset reuse, and complete cleanup. Distinct
allocator instances remain paired with their buffers across moves and swaps.
Normalization and weight-encoding bodies were unchanged in this checkpoint.
The generic `BitWriter::new_in` has a narrow temporary dead-code expectation
until allocator-aware contexts call it; its const Global constructor remains
available to the existing const context constructors.

`FseTable<A>` and FSE weight scratch now use the selected allocator. The
[FSE evidence](evidence/2026-10-09-allocator-fse.json) records 21 focused tests
in each of six configurations: safe, `unsafe-fse`, `unsafe-bitstream`, all
features, no_std safe, and no_std with all unsafe backends (126 executions).
Both no_std builds, formatting, both strict all-target Clippy configurations,
and six exact strict-provenance Miri tests passed.

The tests include all 512 saved count headers and all 12 FSE-compressed weight
payloads among 30 Huffman descriptions. They exercise twelve-bit state widths,
owned/borrowed non-Clone table owners, distinct allocator instances for scratch
and returned weights, each of the two weight allocation failures, failed-copy
source preservation, dictionary singleton copies, allocation-free RLE, and
malformed-input cleanup. The existing Global wrappers preserve earlier callers;
the generic copy helper has a narrow temporary unused-code expectation until
allocator-aware contexts consume it.

`MatchTable<A>` now accepts owned non-Clone allocators, and seed copying accepts
a different allocator type without adopting its storage. Its
[focused evidence](evidence/2026-10-09-allocator-match-table.json) records eight
tests in safe, `unsafe-match-table`, all-feature, and no-default/unsafe-table
configurations; safe/unsafe no_std compilation; formatting; strict all-target
Clippy; and four allocator-specific strict-provenance Miri tests with the
unsafe table backend. New tests compare candidates with Global across resets,
collisions and sentinel values; verify no requests after setup; preserve
destination pointers during seed copying; and verify complete release after
failed construction and swaps between distinct owners.

`MatchChain<A>` and its candidate borrow are allocator-aware. Its
[focused evidence](evidence/2026-10-09-allocator-match-chain.json) records all
15 tests in safe, `unsafe-match-chain`, all-feature, and no-default/unsafe-chain
configurations, safe/unsafe no_std compilation, formatting, strict all-target
Clippy, and four allocator-specific strict-provenance Miri tests. Both initial
allocation ordinals were failed separately, with all partially constructed
owners released. Invalid limits allocate nothing. Candidate order matches
Global with cyclic replacement, visit budgets, future positions and full-width
coordinates. Seed copies from a different allocator type leave source storage
independent and destination ownership intact; swaps move all four heads/node
allocator values with their allocations.

`HuffmanTable<A>`, literal output, and paired serialized descriptions now retain
explicit allocator owners. The [Huffman evidence](evidence/2026-10-09-allocator-huffman.json)
records 71 selected tests passing in each of safe, unsafe-Huffman, unsafe-FSE,
unsafe-codebook, and all-feature builds; ten live-oracle/corpus tests remain
explicitly ignored in each. Both no_std test configurations passed 70 tests.
Five exact strict-provenance Miri cases, two no_std builds, strict Clippy,
benchmark compilation, formatting, and diff checks passed. Frozen source
hashes were checked again before committing.

Custom storage decoded all 30 stored official descriptions and their one- and
four-stream fixtures. Failure injection covers every observed table, output,
and paired-validation request, retaining the existing rule that a replacement
literal table is published only after successful stream decoding. Different
allocator types may supply dictionary tables and output; neither is converted
or copied solely to reconcile ownership. A paired description validates with
a temporary borrow of its own non-Clone allocator and retains its original
pointer and owner. This checkpoint does not claim a new live C or full external
corpus run, nor a public allocator-aware decoder.

Inverse FSE tables, frequency normalization, weight descriptions, and channel
selection now retain their own allocator values. The
[inverse/selection evidence](evidence/2026-10-09-allocator-inverse.json) records
nine inverse-table, eight selector, and 23 entropy tests passing in each of six
configurations: safe, each of the three relevant unsafe backends, all features,
and no default features. Eight all-feature strict-provenance Miri cases passed.
Safe and relevant-unsafe no_std builds, strict all-target/all-feature Clippy,
and formatting passed. A separate snapshot containing only this commit's code
changes also passed strict Clippy, no_std compilation, and formatting, without
the concurrently developed Huffman builder.

The two independent C oracles were explicitly executed in safe and all-feature
builds. Each accepted 521 weight streams, 68 complete Huffman headers, a sparse
weight-twelve header, and a selected new/RLE/repeat frame decoding to 576 bytes.
Custom allocation also preserved all 521 weight-vector outputs and 512 saved
C normalized-count headers. Failure sweeps retain immutable source tables and
release every partial candidate. Distinct clones own fresh transition, initial,
normalization, ranking, and description storage; moving a selected replacement
moves its allocator. Ordinary Clone itself retains its documented caller-owned
panic/allocation boundary. Sequence-context integration is still pending.

The Huffman builder now uses an indexed `MaxHeap<T, A>` and allocator-aware
full, sparse, and standalone description builders. The
[builder evidence](evidence/2026-10-09-allocator-huffman-builder.json) and
[33 command records](evidence/2026-10-09-allocator-huffman-builder-results.json)
record 289 passing test executions. The 22 builder tests passed in nine
configurations, including each of five relevant unsafe features and safe/all
no_std builds. Queue, paired-codebook, fixed-fixture, and formatted-training
checks passed. Both no_std compilations, formatting, strict all-target Clippy,
and seven strict-provenance Miri cases passed.

The safe and all-feature C stream groups each reported 301 checks, including
one documented empty-marker case that bypasses C decoding. Learned-dictionary
CLI interoperability passed separately. The original Module 7 description
fingerprint remains 17806659356530344110. Queue differential tests cover every
small shape, deterministic ties, interleaved operations, non-Copy destruction,
failed growth, and independently owned allocations. Every observed builder
allocation was failed separately. Normal/sparse output retains the original
allocator through fallback; standalone search moves the complete winning
candidate owner. This is a focused builder checkpoint, not a new full-corpus
or complete-codec validation.

Supplied-table sequence emission now accepts independent table and writer
allocator types, requiring neither to implement Clone. Its
[emission evidence](evidence/2026-10-09-allocator-sequence-emission.json) records
ten tests passing in each of six configurations (safe, each relevant FSE or
bit-writer flag, all features, and no default features), plus three exact
all-feature strict-provenance Miri tests. The optional C oracle was separately
executed for safe and all-feature builds: each decoded 27 frames containing
54 supplied-table/repeat sections. Safe/unsafe no_std checks, strict all-target
Clippy, and formatting passed. Source and output owners remain independent
across moves; malformed plans preserve a closed writer, and every observed
writer allocation failure preserves source tables and permits exact retry.

Validated FSE/Huffman views now borrow initialized slices independently of
allocator type. Their [view evidence](evidence/2026-10-09-allocator-table-views.json)
records 23 FSE tests in six configurations. The broader Huffman filter passed
85 tests in each of four std configurations (ten explicit oracle/corpus ignores)
and 84 tests in each of two no_std configurations; these totals include
neighboring module tests. All four new view tests passed all-feature
strict-provenance Miri. Strict Clippy, safe/all no_std builds, benchmark
compilation, formatting, and diff checks passed. No new live C run is claimed.

Owner methods delegate to the same view access algorithm, retaining the existing
safe/unsafe feature behavior. Borrowing and lookup consume no allocator requests
or clones. Tests preserve normalized singleton state widths, inspect every dense
FSE state size, reject extreme bounds, and exercise independent non-Clone output
owners and dictionary fallback followed by frame-local replacement. The views
retain ordinary Rust slice lifetimes; no raw ownership transfer is involved.

`Dictionary<A>` now prepares bytes, Huffman weights, and all four immutable
tables in the chosen allocator family. The
[dictionary evidence](evidence/2026-10-09-allocator-dictionary.json) records
16 tests passing in seven configurations: safe, unsafe-dictionary, unsafe-FSE,
unsafe-Huffman, all features, no_std safe, and no_std with all twelve backends.
Four exact all-feature strict-provenance Miri tests, both no_std compilations,
strict all-target Clippy, and formatting passed with unchanged dictionary
source. Existing official fixtures retain their independent loader assertions;
no new live C or full corpus campaign is claimed here.

The five added tests verify owned non-Clone raw storage, allocation-free empty
raw content, formatted parsing with a borrowed non-Clone allocator, every
prepared state including normalized singletons, all observed allocation-failure
ordinals, malformed-input error preservation and cleanup, and shared lifetime
through final release. The original allocator retains serialized bytes; cloned
values own fresh tables and scratch without assuming equivalence. Standard
`SharedDictionary<A>` uses Global for the counts and entire inline dictionary
object; the custom allocator owns its bytes/tables only. Borrowed public views
and allocator-aware codec attachment remain separate checkpoints.

`LiteralEncoder<A>` now owns its writer, stream, and section buffers, while
Huffman stream emission accepts independent writer/output allocator types.
The [literal evidence](evidence/2026-10-09-allocator-literals.json) and
[32 command records](evidence/2026-10-09-allocator-literals-results.json) report
264 passing executions: eight context tests across nine configurations,
26 codebook tests across six, production publication and entropy integration
checks, safe/all C checks, and four strict-provenance Miri cases. Both no_std
builds, safe/all strict Clippy, and formatting passed.

Global const construction, literal kind/stream decisions, and selected bytes
are preserved. Every observed allocation during fresh and warmed selection was
failed separately; the published codebook stays unchanged, and retry produces
identical output. Prevalidation and failed output reservation preserve both
4X buffers; a later writer failure can leave documented partial scratch.
The C stream groups each report 301 checks including one empty-marker bypass,
with two CLI literal-section/lifetime tests run separately in each backend.
Public block/stream allocator orchestration remains pending.

`DictionaryView<'a>` now borrows validated bytes and entropy without retaining
an allocator type. The sealed `DictionaryHandle` contract statically supports
shared owners and borrowed views; cloning either consumes no allocator request
or allocator clone. The [dictionary-view evidence](evidence/2026-10-09-allocator-dictionary-views.json)
uses an isolated committed baseline, excluding concurrent context changes.
All 18 dictionary tests passed in safe, unsafe-dictionary, all-feature, and
no_std/all-backend builds; both new view/handle tests passed strict-provenance
Miri. no_std compilation, strict Clippy, formatting, and doctests passed.

Final explicit accessor lifetimes permit slices obtained from a temporary view
to borrow the original owner. A successful doctest demonstrates that use; a
compile-fail doctest rejects a view outliving its dictionary. This final lifetime
spelling changes no runtime operation and was checked separately after the
runtime matrix. Content access shares the owning dictionary's existing checked
backend. Capacity is reachable original payload capacity, not memory owned by
the Copy view. Attaching these handles to public codec contexts remains open.

`SequenceState<A>` now owns only allocator-bearing frame overrides and repeat
values, borrowing prepared dictionary entropy per call. Its
[sequence-state evidence](evidence/2026-10-09-allocator-sequence-state.json)
records ten tests in seven configurations, six new all-feature strict-provenance
Miri cases, and passing sequence, dictionary, and streaming integration suites.
Strict Clippy, both no_std builds, benchmark compilation, formatting, and diff
checks passed. Explicitly ignored integration oracles remain identified in the
record; no new live C campaign is claimed for this checkpoint.

Repeat fallback and zero-sequence processing clone no allocator and allocate no
tables. Tests cover independent dictionary/context domains and prepared
singletons. Allocation failures at LL, OF, and ML preserve the existing order
of partial table publication; sink failures may already advance repeats or
output. Reset releases overrides and restores repeats while keeping the selected
allocator. This is the decoder's existing failure contract, distinct from an
encoder's transactional candidate commit.

Raw training now uses the checked setup heap, with allocator parameterization
kept as the next separate item. The
[queue evidence](evidence/2026-10-09-allocator-training-queue.json) and
[23 command records](evidence/2026-10-09-allocator-training-queue-results.json)
report 91 passing test executions, including safe/unsafe-training/all and both
no_std configurations, raw/formatted integration, C CLI checks in safe/all,
and three strict-provenance Miri tests. Formatting, strict Clippy, and no_std
compilation passed.

Before changing the queue, 144 deterministic dictionaries were captured from
the standard BinaryHeap implementation. Their aggregate FNV64 remains
11733226933974065203, covering quotas, clipping, coverage exhaustion, binary
content and ties. The [capture source](evidence/2026-10-09-allocator-training-queue-baseline.rs.gz)
and [passing log](evidence/2026-10-09-allocator-training-queue-baseline.log.gz)
are preserved. Candidate ordering is unchanged; tests add end-index ties and
stale-score reinsertion against the standard heap. Checked peek allocates
nothing, and reinsertion consumes the slot freed by pop without a request.

`SequenceEncoder<A>` now retains every default/current/spare table and scratch
owner. Its [context evidence](evidence/2026-10-09-allocator-sequence-context.json)
records 13 tests in each of six configurations, nine inverse-view tests in each
of three configurations, safe/unsafe no_std compilation, strict all-target
Clippy, formatting, and four all-feature strict-provenance Miri cases. Miri
completed the full ownership/failure sweep in 787.37 seconds. Five std builds
each ignored one optional C test; no-default compiled it out. Explicit C runs
in safe/all builds each decoded the exact 876-byte seven-block frame and a
four-byte prepared-dictionary frame. The earlier emission evidence's summary
was clarified to distinguish its same cfg(std) exclusion; logs did not change.

Dictionary setup copies all three inverse tables through allocator-independent
views before reset/publication. Every one of its six payload requests was
failed separately while preserving an already prepared context. Every observed
fresh or previously committed prepare allocation was also failed separately;
errors expose no partial section or replacement table. Commit swaps complete
owners, and reset/discard retain caches without allocating. One retention test
accounts for 24 live payload allocations across nine dense tables and six
buffers, excluding allocator-internal bookkeeping. The Global const constructor
remains available; custom block/stream integration follows separately.

Public `decompress_in` and `decompress_with_options_in` now construct complete
output in the original requested allocator and allocate all frame-local tables
and literal scratch through fresh clones. Their
[whole-buffer evidence](evidence/2026-10-09-allocator-whole-decode.json) uses an
isolated committed baseline excluding concurrent streaming/training work.
Eight tests passed in nine configurations: safe, each of five relevant unsafe
features, all features, and safe/all no_std. Interop passed in safe/all builds;
the live C decoding differential was explicitly run in both. Two no_std builds,
strict all-target Clippy, formatting, and three all-feature strict-provenance
Miri cases passed.

Custom output matches independent saved C HTTP and Huffman-larger frames,
retains its original allocator identity and pointer across moves, and leaves
only one result allocation after scratch release. Borrowed non-Clone usage,
concatenation, all sixteen skippable magics, allocation-free skip-only results,
limits, checksums, every HTTP truncation, and every observed HTTP allocation
failure are covered. Errors release partial output and tables. The live C
oracle exercises the Global wrapper of the same implementation; custom
streaming and dictionary collection are separate checkpoints.

Public `train_raw_dictionary_in` now retains the original allocator for final
content and uses fresh owners for sample windows, frequencies, candidates and
chosen ranges. Its [training evidence](evidence/2026-10-09-allocator-raw-training.json),
[original matrix](evidence/2026-10-09-allocator-raw-training-results.json), and
[Clippy follow-up](evidence/2026-10-09-allocator-raw-training-clippy.json) report
127 passing test executions. Seventeen training tests passed in five
configurations; raw/formatted integration and C CLI checks passed in safe/all;
four strict-provenance Miri cases, no_std builds, and formatting passed. One
initial shared-workspace lint failure in concurrent formatted finalization is
preserved; both strict Clippy builds passed after that wrapper-only repair.
The exported API additionally passed strict Clippy, no_std, and formatting in
an isolated baseline with no concurrent formatted or streaming changes.

All 144 pre-migration outputs retain their frozen fingerprint, and 184
custom/Global quota/capacity cases match. Every observed allocation ordinal was
failed, including growing window, candidate and selected-range collections and
the final content request. Non-Clone leaf owners and a borrowed non-Clone
aggregate are covered. Successful return keeps only the final content allocation,
with no sample borrow or allocator-changing copy. Miri uses bounded ownership,
frequency, early-failure and final-failure cases; the complete ordinal campaign
runs in normal builds.

Chain/lazy parsing now accepts independent chain, literal, and sequence
allocator types, requiring only `Allocator` for these borrowed owners. The
[parser evidence](evidence/2026-10-09-allocator-match-parser.json) records 25 parser
tests in each of four configurations, public strategy tests in safe/all/no_std,
and eight explicit C full-output comparisons in each of safe/all. Safe and
unsafe-chain no_std compilation, strict all-target Clippy, formatting, and four
all-feature strict-provenance Miri tests passed; Miri took 195.04 seconds.

The tests preserve exact resolver traces, candidate ordering, literals and
sequences across independent allocator domains and moved non-Clone owners.
Failure at either output reservation preserves initialized contents and seeded
chain state; a successful first reservation may retain additional capacity if
the second fails. Invalid shape checks precede allocation or resolver calls.
Resolver errors retain the same documented partial scratch/table state as the
Global implementation. No search policy, visit budget, or coordinate arithmetic
changed. Production copies use individual pushes after reservation.

`Decoder<A, D>` now retains its allocator and a sealed shared/borrowed dictionary
handle type. The [streaming evidence](evidence/2026-10-09-allocator-stream-decoder.json)
records 13 normal tests across ten configurations: safe, six relevant unsafe
features separately, all features, and safe/all no_std. Four all-feature
strict-provenance Miri cases passed. Safe/all integration each passed 93 tests
with four optional external checks explicitly ignored; the new C oracle was
run separately and each configuration decoded eight C frames through both
borrowed and shared custom contexts. Strict all-target Clippy, no_std, benchmark
compilation, and scoped formatting passed. An isolated committed baseline with
only the three checkpoint files also passed Clippy, no_std, and all 13 tests.

The new cases cover nine stored dictionary fixtures across fragmented schedules
and reset replay, every observed allocation ordinal on representative workloads,
independent dictionary/workspace owners, prefix claim/release, limits before
allocation, and changed-window storage release. Prepared tables use temporary
validated views without per-frame copies. Process errors still poison the
context until reset; reset retains allocator-bearing owners and clears partial
frame state. Reachable dictionary capacity is reported even for a borrowed view;
it is not a claim that the context owns those bytes. Standard shared ownership
still has the documented Global outer allocation.

`decompress_with_dictionary_in` collects output using its original allocator,
with scratch owned by a temporary generic decoder. The dictionary may be a
borrowed view or shared owner with an unrelated allocator type; the returned
vector borrows neither dictionary nor input. Its
[collection evidence](evidence/2026-10-09-allocator-dictionary-collection.json)
records four tests across ten safe/individual-feature/all/no_std configurations,
40 dictionary integration tests and an explicit live C dictionary test in each
of safe/all, strict Clippy, no_std builds, and formatting. Three all-feature
strict-provenance Miri tests passed, including the complete observed allocation
failure sweep. Checks used an isolated committed snapshot excluding concurrent
encoder/finalizer work.

Prepared HTTP and raw dictionary fixtures verify original output identity,
independent non-Clone/shared dictionary ownership, result lifetime beyond source
dictionary release, and complete scratch cleanup. Every observed HTTP request
was failed separately without changing dictionary storage. Concatenated and
skippable input, malformed/truncated input, output limits, and empty results are
covered. Empty output consumes no armed payload request. The live C test uses
the Global wrapper of this same collector; custom contexts additionally retain
the streaming checkpoint's separate live C coverage.

`train_dictionary_in` and `finalize_dictionary_in` now retain explicit allocation
ownership across raw selection, entropy descriptions, assembly and the preserved
validation copy. The [formatted evidence](evidence/2026-10-09-allocator-formatted-training.json)
and [30 command records](evidence/2026-10-09-allocator-formatted-training-results.json)
report 211 passing executions: eleven tests in ten configurations, existing
training/Huffman/integration checks, safe/all C interoperability, both no_std
builds, strict Clippy and formatting. Five strict-provenance Miri tests passed
in 24.41 seconds. Public exports additionally passed isolated Clippy, no_std,
unit tests and formatting without concurrent encoder or collector changes.

The [pre-migration capture](evidence/2026-10-09-allocator-formatted-baseline.json)
records 56 complete dictionaries with FNV64 14787186922959212481. Its
[helper](evidence/2026-10-09-allocator-formatted-baseline.rs.gz),
[original source](evidence/2026-10-09-allocator-formatted-baseline-source.rs.gz),
and [result](evidence/2026-10-09-allocator-formatted-baseline.log.gz) preserve
provenance. Custom/Global bytes match; raw and Huffman fingerprints also remain
unchanged. C accepted eight dictionaries and 36 bidirectional frames per
backend, plus the existing FSE/Huffman serialization checks. This is independent
Rust training with C format acceptance, not byte parity with C training policy.

The final dictionary's byte vector retains the original allocator; four prepared
tables own fresh values. Temporary raw content is released before return.
Every observed full-training and finalization allocation was failed separately,
including the final validation copy; malformed assembly also releases all
owners. Returned dictionaries retain no content/sample borrow. Miri covers
bounded owner movement, temporary release, malformed input and final-copy
failure; full ordinal sweeps run in normal builds.

Focused platform execution is recorded in the
[platform report](allocator-platform-validation.md) and
[exact evidence](evidence/2026-10-09-allocator-platform.json). On isolated commit
`dac4a43`, strict-provenance Miri interpreted ten safe and nine all-feature tests
on each of i686 (32-bit little-endian) and s390x (64-bit big-endian). All 38
selected executions passed, plus two preliminary history checks. Both safe
runs include all 23 C checksum vectors through 1 MiB; the identical shared
checksum test was omitted from the all-feature repeat. Interrupted duplicate
attempts remain diagnostic records, not passing-batch claims.

The no-pointer-atomics thumbv6m target passed offline no_std library compilation
and safe/all-feature caller code generation. The caller explicitly requires the
Rc dictionary type and instantiates allocator-aware decoder/dictionary paths.
This does not execute an embedded allocator or produce a linked firmware image.
Miri interpretation is distinct from native/QEMU testing. The report records the
Linux test-helper mutex no-unwind review, exact source snapshot, and remaining
platform limitations; later encoder changes require their own validation.

`BlockEncoder<A, D>` now retains independent literal, sequence, payload, match,
seed and history owners; block destinations may use a different allocator type.
Its [block evidence](evidence/2026-10-09-allocator-block-encoder.json) records five
new allocator tests across 15 configurations (safe, all twelve individual unsafe
features, all features, and no-default), 120 existing dependency tests per
safe/all/no-default build, and public strategy/dictionary tests. Each explicit
safe/all C run decoded six custom frames covering three strategies and both raw
and prepared dictionary views. No_std, strict Clippy and formatting passed;
three strict-provenance Miri cases passed in 70.35, 22.85 and 48.09 seconds.

Every observed setup and encode allocation was failed separately for all three
strategies. Dictionary, workspace and output allocation domains stay independent;
borrowed non-Clone contexts, owner movement, output-failure preservation and
reset recovery are covered. Setup may already have rebuilt a dictionary seed
before inverse-table allocation fails, so reuse requires reset and successful
setup. Failed blocks do not publish output or committed entropy, and history
advances only after accepted emission. The const Global constructor remains.
An isolated earlier-stream baseline required two narrow private-constructor
dead-code allowances; after those attribute-only repairs its Clippy, no_std,
five allocator tests and formatting passed. Public stream/one-shot integration
remains a separate checkpoint.

`Encoder<A, D>` now owns allocator-bearing staging and pending output while its
block workspace retains the original factory. The
[stream encoder evidence](evidence/2026-10-09-allocator-stream-encoder.json) and
[35 command records](evidence/2026-10-09-allocator-stream-encoder-results.json)
report 424 passing executions: eighteen unit tests in sixteen configurations,
streaming/dictionary integration in safe/all, no_std checks, formatting and
strict safe/all Clippy. Each backend's explicit C checks decoded eighteen new
custom-allocator frames, 96 existing dictionary cases, and the broader streaming
interoperability suite. Four strict-provenance Miri tests passed in 7.458 seconds.

A [108-frame capture](evidence/2026-10-09-allocator-stream-baseline.json), with
its [helper](evidence/2026-10-09-allocator-stream-baseline-helper.rs.gz),
[prior stream source](evidence/2026-10-09-allocator-stream-baseline-source.rs.gz)
and [log](evidence/2026-10-09-allocator-stream-baseline-log.gz), retains FNV64
7858392785807785414. Global/custom frames match exactly across strategy,
dictionary and flush schedules. Every observed allocation ordinal in selected
campaigns preserves the existing poison/reset/retry contract and releases all
owners. Prefix claim is distinguished before/after successful header reservation;
invalid reset options do not change a failed context. Initial test-only request
count and lint corrections are retained in the evidence; production behavior
was unchanged. Final ownership documentation and an isolated earlier-public-API
snapshot passed Clippy, no_std, all eighteen tests and formatting.

The [original-size allocation campaign](evidence/2026-10-09-allocator-upstream-campaign.json)
passed in safe and all-feature release builds. C and Rust RDG generation matched
all 89,456,640 bytes at seed 0x12345678, compressibility 0.50 and literal argument
0.0; source FNV64 is 1372079263076317397. Each build checked six custom context
configurations (three named strategies, complete-source Finish and fragmented
Continue/Finish), three Global reference frames and six complete C decodes.
Custom decoders compared all output bytes directly against source. Source and
caller output buffers stayed outside tracked context allocation.

All six frame fingerprints, sizes, allocation counts, requested/live/peak bytes
and retained capacities matched exactly between safe and all-feature builds.
Greedy emitted 30,843,477 bytes (FNV64 6769348192768063082); Lazy emitted 30,257,629
bytes (FNV64 2278302310949511604). Each context released all tracked storage.
The normal smaller-source test repeats domain/counter/byte assertions for every
strategy. Final strict Clippy, scoped formatting and normal tests passed after
a captured concurrent stream unit-test lint correction and diagnostic-only
assertion cleanup. The 87.601/78.089 second large-test durations include other
concurrent work and are not benchmark measurements.

The original C level/worker schedules remain qualified: these six Rust cases do
not reproduce levels 1..6, workers 1..4 or the 60/54/48/24 memtest suffix counts.
The corresponding two inventory anchors advance to partial, not complete.
Static workspaces, estimators and automatic shrinking remain separate contracts.

`compress_in` and `compress_with_dictionary_in` now retain the original output
allocator while block workspace owns fresh clones. Their
[whole-buffer evidence](evidence/2026-10-09-allocator-whole-encode.json) records
six tests across fifteen configurations, 126 existing encode-related tests per
safe/all/no-default build, public strategy/dictionary checks, and 36 exact C
decodes in each safe/all run. The [baseline helper](evidence/2026-10-09-allocator-whole-encode-baseline.rs.gz)
and [captured output](evidence/2026-10-09-allocator-whole-encode-baseline.log.gz)
preserve 36 pre-migration frames with FNV64 5968133839619914848, including the
exact repeating first-48-byte source schedule and content-size/block boundaries.

The allocation campaign fails 106 observed request ordinals in every selected
configuration: empty no/raw/prepared dictionary paths issue 1/2/8 requests;
nonempty paths issue 29/26/40. Every failure releases partial result and scratch.
Tests cover original identity, moved output outliving input/dictionary, owned
and borrowed non-Clone domains, and reserved emission of every header width.
Three strict-provenance Miri cases passed (72.566, 11.441 and 1.903 seconds).
No_std, formatting and strict Clippy passed. A concurrent benchmark lint failure
was preserved and passed after its owner repaired the benchmark; the isolated
five-file checkpoint independently passed strict Clippy, no_std, six tests and
formatting without those concurrent files.

The [caller-owned usage contract](allocator-usage.md) now documents independent
workspace/dictionary allocators, borrowed non-Clone allocation, result ownership,
prefix/reset lifetimes, reachable-capacity accounting and the infallible Global
shared-object exception. Its [documentation evidence](evidence/2026-10-09-allocator-usage.json)
records all ten standalone examples in safe and all-feature builds: three
dictionary examples, two streaming examples, three allocator examples and two
compile-fail cases requiring E0597 for escaping borrowed contexts.

The previous rlib-only standalone rustdoc invocation is invalid on the pinned
compiler, which emits separate full metadata. Its apparent compile-fail passes
were dependency failures, not lifetime evidence. `check_doc_examples.py` now
uses Cargo's actual rlib/rmeta artifact paths; all positive examples pass and
the two lifetime examples fail with their expected diagnostic. `check.sh` calls
this helper for all three usage documents. This changes validation tooling,
not the lifetime or allocation implementation.

The [failure coverage index](allocator-failure-validation.md) connects leaf,
aggregate and public API campaigns to their exact executed records. Selected
successful workloads are replayed with each observed request rejected; focused
existing-state cases additionally check growth, final copies and reset recovery.
The index preserves distinctions between decoder partial state, encoder
candidate publication, and nontransactional dictionary-seed setup. It does not
claim enumeration of every possible input or allocator implementation.

An [independent ownership/safety review](evidence/2026-10-09-allocator-ownership-audit.json)
found no open production allocation-routing, fallibility, detached-owner or
unsafe-access issue in the recorded source. All reviewed source hashes match
the committed codec. Its one documentation finding, the infallible Global
shared-object allocation exception, was corrected and verified. This is source
review evidence; it adds no dynamic, native-platform or compiler-version claim.

All per-owner implementations are now connected through the public codec APIs.
The final focused platform follow-up is recorded in
[allocator-platform-validation.md](allocator-platform-validation.md), and the
[allocator overview](allocators.md) records the final compiler/safety evidence
and distinct remaining contracts. The [final integrated codec matrix](allocator-final-validation.md) now passes
all 58 recorded commands, including 15 complete suites and 814 independent
official-codec comparisons. Timed allocator comparisons remain pending their
separate completion record. The
[upstream allocation plan](upstream-allocation-plan.md) distinguishes ordinary
allocator control from static workspaces, shrinking policies, estimators,
stable buffers, and C numeric compression levels.
