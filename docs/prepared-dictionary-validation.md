# Reusable compiled-dictionary validation

Module 37 is in progress. This report records the evidence gathered so far; it
does not claim native workspace parity, LDM, workers or complete upstream
parity. Design and ownership are in `prepared-dictionary-design.md` and
`prepared-dictionary-analysis.md`.

## Immutable compiled state and consultation

The compiled state owns one resolved profile, content-relative seed tables,
optional prepared entropy and a borrowed dictionary view. Construction rejects
empty content and the optimal families before allocating; the loaded suffix is
capped at the resolved window. Fast, DoubleFast, chain, row and binary-tree
consultation keep per-call sampling separate and read compiled candidates
only after the per-call candidates, with the remaining shared attempt budget.
The binary tree is fully sorted at construction (the
`ZSTD_updateTree`/`ZSTD_insertBt1` load) and consulted through a read-only
`ReferenceTreeView`; the per-call tree keeps its lazy first-query back-fill.

Focused library suites cover admission and release ledgers, empty-content and
optimal-family rejection, window caps, exact Fast seed parity, multi-block and
window-capped consultation, chain/row/tree order, read-only compiled maps,
foreign-family/layout/staged-length rejection, and a retained-plan boundary
test rebuilding plans from resolved records. Items 1-7 and 9-10 added no
unsafe code; item 8 moved the tree owner's checked head/edge reads into the
shared `TreeCells` borrow, so Miri ran the `reference_tree` and `tree_sort`
suites, every tree consultation test, and the compiled tree consultation test
under `unsafe-reference-tree` (20 tests, all passing).

## Public construction, one-shot and coordinator calls

`CompiledDictionary::new` builds the fixed state from a borrowed
`DictionaryView` and a request. `compress_with_compiled(_in)` and
`compress_into_with_compiled(_in)` emit one frame through the compiled profile;
`CompiledOneShotEncoder` owns the state and reuses it across caller-owned calls.
Every call resolves its request against the actual source length and the
dictionary content length and must reproduce the compiled seven-field profile
exactly; a mismatch returns `InvalidParameter` before output admission instead
of using mismatched tables. Caller-owned frames append complete headers and
blocks atomically, preserve guard bytes and report a short destination with
`ResourceLimit`.

Four public integration tests cover allocating/caller-owned byte equality
across Fast, DoubleFast, chain, row and tree levels, formatted-dictionary
entropy and identifier preservation, mismatch rejection before writes,
empty-content and optimal-family construction rejection, and every observed
allocation failure in both the allocating and coordinator caller-owned paths.
Doctests establish the compiled-state/dictionary-view lifetime and input/output
borrow non-overlap.

The compiled frame comparison against Module 36 combined staging is frozen in
`prepared-dictionary-analysis.md#compiled-frame-comparison-item-9`: Fast,
DoubleFast, chain, row and the formatted entropy seed match exactly, while the
tree family records its measured differences explicitly.

## Guarded native schedules

Thirteen typed compiled-dictionary cases run against normal and sanitized
pinned v1.5.7 CDict controls: Fast, DoubleFast, chain and BinaryTreeLazy2,
raw and formatted attachments, empty/no-match and multi-block inputs, a
window-capped raw dictionary, and unsupported Optimal/Ultra2 construction.
Both builds produce identical thirteen rows and frames; Rust frames decode
natively, native frames decode in Rust, every observable native dictionary
control differs from its fresh no-dictionary control, and the window-capped
Rust case equals the dictionary-free parameterized bytes (native window
adjustment differs and is not claimed as byte identity). Eleven supported and
two rejected rows are retained, with independent destination guards.
Evidence: `evidence/2026-11-03-compiled-dictionary-native-platform.json`.

## Concrete no-atomics ARM emission

Compiled allocating, caller-owned and coordinator paths run on the host under
safe and all features. Library checks and release LLVM IR/object emission pass
for `thumbv6m-none-eabi` with safe and every opt-in storage backend together;
the IR and object artifacts are retained. Foreign Miri was not run because this
item added or modified no unsafe code, per the standing direction.

Remaining Module 37 items are the full feature and release matrix, timing with
CPU barriers, and final reconciliation.
