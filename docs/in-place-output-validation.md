# Bounded in-place decompression qualification

Module 28 remains in progress. Focused stages below are independently qualified;
complete feature/release regression, measurements and final reconciliation remain
separate gates. The implementation remains dictionary-free.

## Checked storage

The checked resumption constructor rejects reversed or out-of-range frame/output
coordinates before constructing history. It treats only the coordinator-supplied
written prefix as eligible history. Existing-frame match distances and window
limits remain enforced after reborrowing. No history clone is introduced.

`InPlaceBuffer` owns one exclusive slice and checked index coordinates. Raw blocks
permit actual overlap through safe `copy_within` or the independent checked
`unsafe-in-place-buffer` path. RLE protects the payload start. Compressed regions
use disjoint split borrows and checked endpoint publication. Rejected operations
preserve coordinates and perform no writes. Plain coordinates use one shared
representation; only raw movement selects the new unsafe feature.

The focused storage receipt records four safe/move/history/combined selections,
with nine history and six buffer tests in each, no_std, strict Clippy and format.
The private storage is initially test-gated until its coordinator is integrated.
Earlier standalone movement qualification is retained separately in the prior
session; final integrated platform/full-feature checks remain separate.

Evidence: `evidence/2026-10-27-in-place-storage.json`.

## Private frame coordinator

The frame loop copies metadata into values before writes. Compressed blocks
resume history in the mutable left split and retain their encoded payload in the
immutable right split, then publish the actual checked endpoint after borrows
end. Raw blocks permit overlap; all later encoded bytes remain protected. Frame
boundaries reset match eligibility, and frame-local output validates content size
and checksum. Scratch owners retain existing fallible allocation.

Six private-loop tests pass in safe/move/history/combined builds, including real
raw overlap, RLE/empty boundaries, native entropy fixtures at sufficient margin,
concatenation/skips, compressed blocks, malformed ranges and insufficient fences.
The coordinator remains test-gated pending the public API; complete regression
and independent native same-allocation qualification are separate.

Evidence: `evidence/2026-10-27-in-place-engine.json`.
