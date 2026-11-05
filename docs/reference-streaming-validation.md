# Request-backed reference streaming validation

Module 40 implements a bounded request-backed owner over the existing
reference streaming state machine. See [analysis](reference-streaming-analysis.md),
[design](reference-streaming-design.md) and [usage](reference-streaming-usage.md).
The pinned test oracle is facebook/zstd v1.5.7 at
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`; it is never linked into the Rust codec.

## Public and focused checks

Eleven public tests in `tests/reference_stream.rs` cover:

- Six explicit families over multiple blocks and beyond the retained window,
  with known and unknown sizes, checksum and caller output guards.
- Reset-time profile changes at source sizes 0, 1, 255, 256, 1024, 1025,
  65,791, 65,792 and 131,073; content-size emission enabled/disabled;
  unknown-size reset and initialized-parameter introspection.
- A source hint that reduces the window without becoming an exact pledge.
- Flush, zero output capacity, one-byte output, Finish idempotence, directive
  errors, pledge mismatch, poisoning and reset recovery.
- Unsupported profiles rejected without allocation and deferred admission
  failure before header output after an initially admitted small pledge.
- Every allocation ordinal of a successful reference Greedy frame rejected
  in turn, with exact allocation error, poison, reset, deterministic recovery
  bytes and complete allocator release.
- Direct prefix-only construction for all six families, one-frame consumption,
  prefix-free re-resolution on the next frame, replacement/detach and pending
  versus claimed prefix reset boundaries.
- Formatted-prefix entropy, optional dictionary ID, checksum and a later frame
  decoded without a dictionary.

Focused existing compiled-stream integration suites also pass after factoring
the shared constructor. Initial focused Clippy, formatting and host no_std
checks pass. The full module gates are recorded separately below; focused
results alone do not complete a milestone.

## Automatic family changes across reused frames

A supplementary safe/all harness holds a single level-four request while
resetting between known sizes 0, 16,384, 65,536, 200,000, an unknown-size
4,096-byte input, and known size 513. Expected Greedy/DoubleFast family codes
come from the independently captured pinned `tests/upstream/parameters/presets.tsv`,
not the resolver under test. Every reused frame selects the expected family,
is byte-identical to a fresh-context frame under the same options, and decodes
to its exact source. This checks that prior parser geometry and entropy do not
leak across an automatic family change. The safe/all six-row observations are
identical; focused Clippy and formatting pass after the recorded harness-only
lint correction. This external harness is supplementary to the 11 repository
integration tests, not part of their default-suite count and not additional
native frame interoperability evidence.

Receipt and reproducible harness/logs:
`evidence/2026-11-05-reference-streaming-profile-switch.json` and its archive.

## Guarded pinned-native schedules

`tests/reference_stream_native.rs` exports Rust frames and replays the
external oracle's output. Six explicitly requested families each run six
scenarios, with a second session after each first frame: 72 frames total.

| First frame scenario | Pledge | Prefix | Next session |
| --- | --- | --- | --- |
| Empty input | known zero | none | known 1,302-byte source |
| 513 bytes | known, visible | none | known 1,302-byte source |
| Repeated text across a content-size width boundary | known, hidden | none | known 1,302-byte source |
| 163,840 bytes across blocks/window | unknown | none | known 1,302-byte source |
| 513-byte slice from raw content | known, visible | raw | no prefix, known 1,302-byte source |
| Prepared-dictionary text | known, visible | formatted | no prefix, known 1,302-byte source |

Both implementations initialize with an empty Continue so native's initial
Finish auto-pledge does not change the test's size semantics. Each side uses
317-byte input fragments and 17-byte output windows with destination guards.
The normal and ASan/UBSan native programs produce identical 72 rows and frame
files. Both programs decode every Rust and native frame using its actual
attachment and verify destination guards. Rust safe/all builds decode all
72 native frames with exact source equality and content-size/checksum checks.
For dictionary-free native frames, the oracle verifies the window against the
Rust resolved window or the native single-segment source-size window.
Formatted-prefix profile identity and native frame-byte equality are not
claimed. Sanitizer leak detection is disabled in this host campaign; allocator
release is checked separately by Rust tests.

The external helper, scripts, source manifests, raw logs and frame files are
retained in `evidence/2026-11-05-reference-streaming-native.tar.gz`, with the
receipt in `evidence/2026-11-05-reference-streaming-native.json`. The ignored
native test is run explicitly by this campaign; ignored ordinary-suite tests
are never credited as passes.

## Full feature, release and platform matrix

All 79 commands pass on the frozen final codec/test source: safe/all and each
of 22 isolated unsafe features run complete tests, host no_std and strict
Clippy; safe/all release suites, safe/all atomic-free ARM checks and release
LLVM IR/object emission, and formatting also pass. Every full/release suite
reports 129 result groups, 1,878 passed and 81 ignored, with zero failures.
The native campaign runs its ignored adapter explicitly; other ignored tests
remain uncredited.

For all-feature host no_std, the command explicitly enables all 22 unsafe
features while disabling defaults. Using `--all-features` there would also
enable `std`; this campaign avoids that ambiguity. Atomic-free
`thumbv6m-none-eabi` uses the same explicit feature list with
`-Zbuild-std=core,alloc`, including concrete release IR and object output.
The first safe-release attempt exhausted temporary disk space during
compilation/linking. Its failure log is preserved separately. After removing
the completed debug build cache, the controller verified all 72 prior log
hashes and the immutable source, then reran release and the remaining gates.
Only successful final attempts count toward the 79-command result.

The source manifest and object hashes are retained in
`evidence/2026-11-05-reference-streaming-feature-matrix.json`; the matching
archive contains the controller and every raw log. No Miri ran because this
module added or changed no unsafe code, following the standing directive.

The independent timing comparison is recorded in
`reference-streaming-benchmarks.md` and its 2026-11-05 receipt: 18 workloads,
11,520 samples and byte-identical safe/all repeat ledgers. It compares distinct
reference and named policies and makes no universal speedup claim.

## Bounded scope

Only Fast, DoubleFast, Greedy, Lazy, Lazy2 and BinaryTreeLazy2 are admitted,
subject to their existing table/window bounds. Reference requests are fixed
for the owner's lifetime; geometry is resolved per frame, not per block.
Initial Finish does not infer a pledge. Headers use a legal window descriptor
rather than native's optional single-segment optimization. Prefix parameters
use matchable content length and conservative window-capped suffix staging.
Optimal/Ultra/Ultra2 streaming, mutable requests and parameter-only resets,
native CCtx workspace attachment/loading reuse, LDM, workers and complete
upstream test migration remain open.
