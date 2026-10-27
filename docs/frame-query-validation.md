# Checked frame query qualification

Module 30 is in progress. Focused gates below are complete; full feature/release
regression, measurements and final reconciliation remain separate gates.

## Shared structural traversal

The existing margin traversal retains copied standard headers or skippable
variant/length metadata. The same checked cursor validates encoded extents,
block headers and checksum-byte presence. No second parser or payload allocation
is introduced. Plain copied records share their safe/unsafe representation;
the existing independent byte-cursor feature selects traversal operations.

Safe, byte-cursor and all-feature configurations pass nine private unit tests,
the existing margin documentation example, true no_std and strict Clippy, plus
formatting (13 commands). This preserves the predecessor's margin behavior.
Evidence: `evidence/2026-10-27-frame-query-layout.json`.

## Public metadata, sizes and borrowed payloads

FrameInfo owns copied validated structural metadata with read-only accessors;
FrameKind distinguishes standard headers from skippable variant/payload sizes.
First-frame queries ignore trailing input. Concatenated size and bound queries
validate every encoded extent, even after an unknown-size frame. Unknown size is
Option::None; errors remain distinct. Checked u64 arithmetic reports overflow
without native reserved numeric sentinels or pointer-width truncation.
The skippable predicate recognizes four marker bytes only; the payload query
validates the entire first frame and returns a borrow of its payload.

Nine unit, seven public and four documentation tests pass in safe/cursor/all
configurations, alongside no_std, strict library/test Clippy and formatting
(16 commands). A compile-fail example proves the payload cannot outlive input.
Tests distinguish structural validity from entropy/checksum/advertised-size
truth, cover every skippable variant, full-u64 sizes, overflow, malformed tails
and a small encoded stream whose logical bound exceeds 4 GiB.
Initial test-call and lint preparation rejections are retained; they precede
the passing final focused snapshot and receive no execution credit.
Evidence: `evidence/2026-10-27-frame-query-public-api.json`.

## Native comparison and original assertions

Pinned v1.5.7 normal and ASan/UBSan helpers query 10,818 actual inputs. Safe,
byte-cursor and all-feature Rust captures agree and explicitly replay the final
ignored native adapters. Of these inputs, 10,268 complete query rows agree;
550 differences are retained and classified from actual input bytes: 519 validate
malformed tails after native early unknown-size returns, 17 admit structurally
representable window values above native limits, and 14 distinguish full-u64
values/checked overflow from native sentinels or wrapping aggregate arithmetic.
No differing row is silently counted as parity.

The original test_decompressBound input bytes, compression/flushing operations
and size/bound assertions are reproduced, including 32,769 flushed blocks and
an above-4-GiB logical bound. The helper reuses its harness buffer/context across
some examples, so identical native context allocation lifetimes are not claimed.
A separate instrumented original fuzzer executes the preceding original history
unchanged and captures single-frame, unknown-size and four-frame-plus-skippable
inputs. Normal/sanitized payloads agree; three Rust backends verify complete
output and original exact/tight/truncated assertions on those captured bytes.
An independent audit matches all 176 compiled upstream C/header/assembly files
against the pinned tarball, including test data-generation dependencies.

Original group 948 queries the full compressed buffer capacity instead of actual
frame size. It obtains the native error sentinel, whose unsigned comparison
passes the original lower-bound assertion. The useful exact-frame-length Rust
bound is explicitly supplemental. This group remains partial; the error-sentinel
comparison is not credited as a meaningful bound guarantee.

Preflight Clippy rejected a native adapter constant chunks_exact operation.
Checked as_chunks with an explicit empty-remainder assertion resolves that
warning; all three native replays and preflight pass again. Historical rejected
source/logs remain retained. Production source was unchanged.
Evidence: `evidence/2026-10-27-frame-query-native.json` and
`evidence/2026-10-27-frame-query-history.json`.

## Corpus and foreign targets

All deterministic decodecorpus fixtures and their concatenation with skippable
frames pass structural metadata, size/bound, truncation and malformed-tail checks
in safe/cursor/all configurations, with Clippy and formatting (five commands).
Existing decoding corpus assertions remain intact.

Four actual strict-provenance Miri runs cover i686/s390x safe/all builds, including
the full 32,769-block small-input example whose bound exceeds 4 GiB. Safe/all
thumbv6m core-only checks and actual LLVM IR/ARM objects also pass (eight commands).
The component copies the exact production cursor, frame, error and layout modules;
its production library requires only core, while tests use alloc for fixtures.
This proves the query component has no allocator dependency. The full codec
continues to require no_std plus alloc. These are interpreter/code-generation
results, not execution on foreign hardware.
Evidence: `evidence/2026-10-27-frame-query-platform.json` and
`evidence/2026-10-27-frame-query-corpus.json`.
