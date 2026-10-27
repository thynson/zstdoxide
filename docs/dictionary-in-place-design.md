# Persistent dictionaries in one-shot in-place decompression

This module follows bounded dictionary-free in-place decompression. Its ownership
analysis and task decomposition must be committed before implementation. Preserve
Module 28's frozen production/check/measurement snapshots. No new dictionary API
is admitted until the predecessor closes.

## Contract and source ownership

Add `decompress_in_place_with_dictionary(buffer, input_start, dictionary, options)`
and `decompress_in_place_with_dictionary_in(buffer, input_start, dictionary,
options, allocator)`. The Global wrapper accepts `&SharedDictionary`; the generic
wrapper accepts `&D` where `D: DictionaryHandle`, plus `A: Allocator + Clone`.
The borrowed dictionary can have a different allocator type from scratch.
Its handle and allocator are not cloned by this call.

The sole mutable slice still owns input and output. Input is its suffix, output
starts at zero, and all Module 28 dynamic unread-input fences remain enforced.
The immutable dictionary is separate storage; Rust's borrows prevent a live
DictionaryView from aliasing any part of the mutable buffer. Compile-fail examples
must demonstrate this boundary rather than inventing a pointer-based API.

One selected raw/prepared dictionary applies to every standard frame in the
concatenation. Nonzero IDs must match; omitted IDs still use the selection.
Skippable frames do not alter selection. Each standard frame resets output history
eligibility and repeat offsets from the selected prepared dictionary, if any.
Borrowed initial entropy remains immutable fallback; frame-local overrides remain
allocator-owned and are released at frame completion/error. A later frame must
not inherit overrides or decoded output eligibility from an earlier one.

Empty input, invalid input index, window/capacity bounds, checksum/content-size
validation, unchanged successful suffix, provisional failure writes and unsupported
format rules match the existing in-place API. Dictionary mismatch precedence is
explicitly documented without promising native numeric errors. Dictionaries do
not consume output capacity. No complete input/output/history/dictionary clone is
permitted. Dictionary-free behavior and public wrappers remain unchanged.

## Data structures and upstream counterparts

`InPlaceBuffer` retains its unique slice and validated scalar coordinates. It
needs no dictionary field and no additional unsafe family. `SliceHistory::resume`
retains its bounds contract. Extend the existing `DictionarySliceOutput` with a
checked resumption constructor composing that history with a borrowed
`DictionaryView`. The wrapper owns no allocations and requires no artificial
unsafe representation; dictionary access and slice-history access continue to
use their independently selectable existing features.

The frame coordinator receives `Option<DictionaryView<'_>>` and uses the existing
borrowed Huffman/FSE fallback contracts. Initial dictionary distances can exceed
the frame window while the current frame position has not passed that window,
exactly as in the already qualified dictionary-output sink. Copying can cross
from dictionary bytes into the current frame, but never into a previous frame.
Temporary compressed payload/history split borrows end before publication.

Pinned reference: facebook/zstd v1.5.7 revision
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`, especially
`ZSTD_decompress_usingDDict`, `ZSTD_decompressMultiFrame`, `ZSTD_decompressFrame`,
`ZSTD_decompressBlock_internal`, and dictionary-aware sequence execution.
The native helper remains a standalone test oracle outside the repository.

## Qualification

Exercise raw/prepared/borrowed/shared dictionaries, nonclone scratch owners,
independent dictionary/scratch allocator types, prepared fallback/repeat tables,
dictionary-to-frame match crossing, window expiry, omitted/mismatched IDs,
concatenation/reset, malformed/truncated inputs, exact/smaller margins and guards.
Inject every scratch allocation failure and verify no dictionary ledger changes.
Raw/RLE/skippable paths must not request codec payload allocations merely because
a dictionary is selected.

Native normal and ASan/UBSan tests must pass genuinely overlapping source/output
pointers to the selected-dictionary decoder, retain both guards and full-byte
outputs, and separately record semantic differences. Existing dictionary fixtures
retain their pinned source attribution. Apply all relevant deterministic corpus
fixtures, with supplemental in-place layouts explicitly distinguished from original
upstream assertions; do not upgrade unrelated groups to complete.

Run safe/isolated/all full and release regression, true no_std, strict Clippy,
foreign strict-provenance Miri on i686/s390x and concrete thumbv6m core+alloc output.
Freeze identical layouts and compare in-place/direct dictionary decoding, source
restoration cost and independent allocator ledgers using retained executables.
Rebuild safe/all independently and measure after actual CPU barriers. Only after
source-bound reconciliation should the checklist move to progress history.

Single-use in-place prefixes, incremental overlap, stable-input contracts, external
workspaces, legacy/magicless frames and complete upstream parity remain outside
this bounded module.
