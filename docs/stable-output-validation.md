# Stable borrowed incremental output qualification

`SliceDecoder` holds one exclusive destination borrow across input fragments.
It writes raw, RLE and compressed blocks directly into checked `SliceHistory`,
uses earlier bytes of the current frame as match history, and never allocates
an output vector or ring. Temporary input, literal and entropy storage remains
fallible. The fixed capacity and output allowance are independent of the
advertised-window limit. Dictionaries, prefixes, stable input and overlapping
in-place decoding remain separate contracts.

## Staged ownership, state and API evidence

`evidence/2026-10-26-stable-output-storage.json` records ten focused commands
for the checked prefix/reset operation and unchanged one-shot behavior under
safe, isolated slice-history and all features, plus strict Clippy and true
no_std. Reset removes all old bytes from output and frame-local history without
erasing the caller's allocation. Both storage backends share the same checked
invariants and public behavior.

`evidence/2026-10-26-stable-output-engine.json` binds ten commands to the private
state-machine snapshot before public admission. Eleven private engine tests
cover fragmented input, frame boundaries, output/limit enforcement, truncated
and malformed streams, failed-state rejection, resets and allocation failure.
The separately archived public stage enables the module and exposes its API;
there is no second production algorithm hidden behind its test configuration.

`evidence/2026-10-26-stable-output-public-api.json` records sixteen commands.
Safe, isolated slice-history and all builds each pass focused API and existing
caller-output/decodecorpus regressions, the native case replay, eleven Rustdoc
examples including lifetime compile failures, true no_std and strict Clippy.
Formatting also passes. Ten public integration tests exercise every split of
an independent compressed HTTP frame, empty calls, raw/RLE, concatenated and
skippable frames, exact/short output, independent window limits, every truncated
prefix, mutation comparisons, failure poisoning and resets from partial phases.
Every observed scratch allocation ordinal is failed separately and followed
by reset/recovery. Nonclone allocator borrows release all scratch; raw/RLE
sessions issue no codec payload allocation requests.

The caller cannot replace or mutate the borrowed destination between calls.
A view returned by `output()` cannot remain live across a mutating session
operation. Output is provisional until its frame's checksum/content size is
validated; a failed session denies output access and requires reset. Capacity
failure is `ResourceLimit`, with bytes beyond the allowance unchanged.

## Independent pinned native stable-output checks

`evidence/2026-10-26-stable-output-native.json` retains the helper source, native
build identities, executables, complete case results and output bytes, inputs,
Rust replay logs and a conservative upstream inventory mapping. The restored
pinned reference tree was checked against all 418 recorded source identities;
recorded CRLF normalization and symlink-target hashing recover the pinned Git
blob content. Downloaded reference source remains outside the repository.

Normal and AddressSanitizer/UndefinedBehaviorSanitizer native executables each
run 1,196 cases. Rust safe, isolated slice-history and all-feature builds each
run the 1,193 common cases. They cover all 96 official generated decodecorpus
frames, independent HTTP, empty/skippable/concatenated frames, corrupt checksum,
truncation and empty input, using unique zero/exact/short/excess capacities and
one-byte, seven-byte and complete-input schedules. Successful bytes match their
independent originals and real destination guards survive. A normal committed
test also decodes all 96 generated frames with exact capacity at three input
fragmentation widths.

The three additional native-only cases change the output position, pointer or
capacity after an initial call and receive `dstBuffer_wrong`. Safe Rust's
borrowed API excludes those mutations; the compile-fail examples qualify its
corresponding ownership contract. Native `dstSize_tooSmall` corresponds to the
bounded Rust `ResourceLimit`, without claiming identical error precedence or
provisional output. Native workspace measurements are retained as observations,
not assertions about Rust's allocation layout. Four named upstream stable-output
groups remain partial because C ABI/workspace checks, runtime mode switching
and original randomized data generation are not equivalent Rust assertions.

## Foreign interpretation and no-atomics code generation

`evidence/2026-10-26-stable-output-platform.json` records four strict-provenance
Miri executions, safe/all on i686 and s390x. Real compressed frames exercise
fragmented direct history, concatenation, reset, exact output, bounded failures
and sentinel protection. These are foreign-width/endian interpreter executions,
not runs on native foreign hardware.

Two thumbv6m core+alloc checks and two concrete release code-generation builds
pass. Retained LLVM IR contains an actual `slice_decoder_codegen_probe` that
constructs, processes and finishes a public session; retained objects are
verified little-endian 32-bit ARM ELF. The executed test source is retained;
the public candidate applies only formatting to that test. Production file
hashes match exactly. No source-level type-check-only probe is substituted for
concrete code generation.

## Exact-selector streaming fuzz corpus

`evidence/2026-10-26-stable-output-streaming-corpus.json` records the additive
adapter added after the full-matrix source freeze. It preserves the upstream
stable selector and its conditional parameter consumption instead of consuming
and ignoring that flag as the earlier buffered adapter did. Seven normal tests
pass under safe, all and every one of twenty isolated features; strict all-feature
Clippy and formatting also pass. The full matrix qualifies the frozen base;
these twenty-two runs separately qualify the additive test code on every backend.

Every one of the 12,462 pinned corpus files (80,339,212 bytes) runs under Rust
safe/all release and the unmodified pinned native non-stateful fuzz target in
normal and ASan/UBSan builds. There are zero skipped files. Per Rust run, 620
inputs select stable output and 11,842 select buffered output. Outcomes total
82 completed streams, 496 incomplete streams and 11,884 malformed/limit errors;
errors are accepted fuzz outcomes, not claims that those inputs are valid frames.
Safe/all per-file ledgers match exactly. Every native/Rust reserved prefix,
stable flag, capacity, initial output size and first input size also matches.
Native and Rust error codes, progress/call counts and provisional bytes are not
asserted identical.

The fixed destination uses the exact selected capacity and real disjoint guards;
errors poison the Rust session. Prefix and suffix guards also protect buffered
output. The adapter retains a 2 MiB input bound, 128 MiB advertised-window bound
and 10 MiB aggregate buffered-output bound; non-stateful contexts match the
native target's default build. Stateful context reuse and unrestricted resource
behavior remain partial. Its archive SHA-256 is
`7434eba13e322a2a9b504ddb57217cf3568d360df309e3c9003956bd449e1de8`.
This is a separately pinned rolling corpus snapshot, not established membership
in the v1.5.7 source tag. Input hashes, selector/outcome ledgers, native executable
identities and conservative inventory mapping are retained in the evidence.

Full upstream migration remains open, including unsupported advanced encoder,
workspace, threading, legacy and CLI behavior.

## Complete regression, measurements and reconciliation

`evidence/2026-10-26-stable-output-feature-matrix.json`
records the complete suites: 1,580 passed and 57 explicitly ignored
tests across 80 result groups in each configuration. All twenty isolated
unsafe features and safe/all configurations have complete regression, true
no_std and strict Clippy qualification. Formatting and complete all-feature
release pass; safe release is also complete.
Ignored external or unsupported assertions are not counted as passed tests.

`stable-output-benchmarks.md` reports all 22 primary backends and independently rebuilt
safe/all repeats on identical deterministic workloads and allocation ledgers.
Source, fixtures, executables, CPU barriers and raw samples are retained. These
fixed-order host measurements do not establish a universal unsafe speedup.
`evidence/2026-10-26-stable-output-closure.json` rehashes this module's evidence and binds
the integrated production source to final matrix, native/platform, corpus where
applicable, and measured executable source identities. Previous milestones are
linked through their closure receipts; historical checks are not relabelled as
fresh executions. Complete upstream migration remains open.
