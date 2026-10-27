# Persistent dictionaries in in-place output qualification

Module 29 is in progress; final feature/release regression, measurements and
source reconciliation remain separate gates. Focused checks below are complete.

## Checked history composition and frame selection

The private coordinator borrows an optional DictionaryView. Every standard frame
validates selection/ID, resets repeat offsets and uses immutable prepared entropy
fallback with local overrides. DictionarySliceOutput::resume composes checked
existing output coordinates with an immutable view; it copies no history and
clones no dictionary handle or allocator. Dynamic compressed split borrows and
raw overlap preserve the predecessor's unread-input fences.

Five safe/movement/history/dictionary/all configurations pass two new resumption
and eight loop tests, true no_std and strict Clippy, plus formatting (21 commands).
This includes dictionary-free regression, 13 pinned raw/prepared fixture pairs,
concatenation reset, invalid dictionary offsets/IDs and preserved guards.
Evidence: `evidence/2026-10-27-dictionary-in-place-engine.json`.

## Public ownership and allocation contracts

The Global shared-handle wrapper and generic allocator/view wrapper each preserve
caller output ownership. Seven public contract tests, eight loop tests and three
documentation examples (including two required compile rejections) pass in all
five focused configurations with no_std, strict Clippy and formatting (26 commands).
Exact bounds, prepared resets, omitted/mismatched IDs, all HTTP truncations and
selected mutations, independent allocator types and non-Clone borrowed owners
are exercised. Every observed scratch failure ordinal releases storage and leaves
dictionary accounting unchanged. Selected raw/RLE/skippable paths allocate no
codec payload storage.
Evidence: `evidence/2026-10-27-dictionary-in-place-public-api.json`.
Usage: `dictionary-in-place-usage.md`.
