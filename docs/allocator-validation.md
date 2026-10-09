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

The owner-by-owner implementation and failure-injection results will be added
as their focused checks complete. Whole-codec custom allocation, 32-bit and
big-endian execution, and no-pointer-atomics compilation remain open. The
[upstream allocation plan](upstream-allocation-plan.md) distinguishes ordinary
allocator control from static workspaces, shrinking policies, estimators,
stable buffers, and C numeric compression levels.
