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
