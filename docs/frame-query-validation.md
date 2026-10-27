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
