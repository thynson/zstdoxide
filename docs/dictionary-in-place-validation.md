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

## Independent native layouts and complete dictionary corpus

A standalone v1.5.7 normal/ASan+UBSan helper supplies a by-reference DDict and
source/destination pointers into one real guarded allocation. All 1,836 layouts
agree across the two native builds. Dictionary bytes, outer guard bands and the
configured output allowance are checked. Five Rust capture backends and the final
explicitly invoked ignored adapter agree on all Rust results and all common
success bytes. Sixty-six empty-input layouts preserve UnexpectedEof versus native
success; three p-window-plus-one layouts preserve strict dictionary expiry after
frame position exceeds the advertised window. An independent direct-output probe
confirms the same existing Rust policy and checks native output against the
pinned expected bytes. Native changed unused successful suffixes in 53 layouts;
Rust separately verifies its stronger unchanged-success-suffix contract.

An observation helper initially supplied Dictionary where the sealed handle
contract requires a view/shared owner; the compile rejection is retained and
earns no execution credit. Corrected observation code uses the existing view.
No production fix was required. Native helpers remain external oracles, and
generated layouts are supplements, not newly ported original upstream groups.
Evidence: `evidence/2026-10-27-dictionary-in-place-native.json`.

All 12,063 hash-pinned dictionary_decompress corpus files run with zero skips in
safe/all release. Persistent prepared/loaded branches additionally compare suffix-
input in-place output with direct, stable and buffered paths using real guards.
Prefix selections retain their existing adapters and remain outside this in-place
dictionary supplement. All selectors and historical bounded results are retained:
11,768 trained, 295 explicit TrainingFailed fallbacks and 20 successful decodes.
Per-file selector/size/capacity/training/success ledgers match. Independent Rust
training is not claimed byte-identical to native fastCover.
Evidence: `evidence/2026-10-27-dictionary-in-place-corpus.json`.

## Foreign interpretation and concrete code generation

Four actual strict-provenance Miri executions cover i686/s390x safe/all builds,
including dictionary crossings, prepared literals/repeats, frame resets, raw
overlap and guards. Safe/all thumbv6m core+alloc checks and actual LLVM IR/ARM
objects also pass, with the overlap operation instantiated. Eight completed
commands are retained. The original observation probe incorrectly passed an
owned Dictionary; its rejected compile remains archived. The corrected probe
uses an immutable view, and already completed Miri/core checks were preserved.
Production source is unchanged. This is interpretation/code generation evidence,
not execution on foreign hardware.
The full matrix initially rejected the test fixture tuple type under strict Clippy.
A named type alias resolves the warning without changing runtime operations; all
eight platform commands were rerun on that final test source. Prior receipts and
failed logs retain their identities.
Evidence: `evidence/2026-10-27-dictionary-in-place-platform-final.json`.
