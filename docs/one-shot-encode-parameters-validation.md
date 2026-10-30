# Parameterized dictionary-backed one-shot compression validation

Module 36 is in progress. Only the internal combined-staging engine, its
DoubleFast extension and the Greedy chain-and-row family extension are
qualified and integrated. Public parameterized dictionary APIs, the remaining
profile families, native/corpus/platform validation, the complete
feature/release regression, measurements and reconciliation remain open.

## Internal combined staging for the Fast profile

A dictionary-aware plan resolution feeds the pinned reference's dictionary-size
tier input. Inside one call the in-window suffix of dictionary content is
staged contiguously before the input in per-call scratch, the Fast hash table
is seeded with every staged key position, and the existing parser runs over
the combined borrow unchanged: match extension crosses the staged boundary
contiguously, the existing candidate floor bounds every emitted offset by the
resolved window, and combined-space positions produce decoder-correct offsets
by construction. Frame flags follow the parameterized family (known content
size, no checksum) plus the dictionary ID field between the optional window
byte and the content size. The staging copy is released before return; no
workspace survives the call.

Thirteen focused commands pass: the complete library suite, true no_std
compilation and strict Clippy under safe, the isolated match-table and
dictionary backends the combined path exercises, and all features, plus
formatting. Every full suite reports 1117 passed
and 25 ignored across
1 result groups. Five unit tests cover
dictionary reach with deterministic noise, exact dictionary-free byte equality
for empty and empty-dictionary edges, staged-suffix window expiry, explicit
unsupported-feature rejection for the unstaged profiles, and prepared
dictionary identifier emission with roundtrip decoding.
Evidence: `evidence/2026-10-29-one-shot-encode-parameters-engine.json`.

## Combined staging extension to the DoubleFast profile

`DoubleFastTables::seed_prefix` seeds both the long and the short hash table
with every staged key position of the combined borrow, and
`DoubleFastMatchState::from_parts_with_prefix` binds the prefix-relative
geometry: the source-length check accounts for the staged suffix, parsing
starts at the prefix boundary, and the existing candidate floor keeps every
emitted offset within the resolved window. Either seeded table can therefore
offer dictionary candidates while the existing parser still runs unchanged
over the combined borrow. A nonzero prefix now admits the Fast and DoubleFast
profiles; every other profile still rejects combined staging with
`UnsupportedFeature` instead of silently dropping the dictionary.

Thirteen focused commands pass: the complete library suite, true no_std
compilation and strict Clippy under safe, the isolated match-table and
dictionary backends the combined path exercises, and all features, plus
formatting. Every full suite reports 1118 passed
and 25 ignored across
1 result groups. One new unit test proves DoubleFast dictionary reach,
output shrink against the dictionary-free emission and roundtrip decoding with
deterministic noise through level 0; the unsupported-profile rejection test
moved to levels that still reject.
Evidence: `evidence/2026-10-30-one-shot-encode-parameters-double-fast.json`.

## Combined staging extension to the Greedy chain and row families

`GreedyTables::seed_prefix` seeds every staged key position into the selected
chain or row owner - the chain through one insert per position, mirroring
upstream dictionary loading, the row under the same fresh salt the parser
searches with so seeded tags match every later lookup - and
`GreedyMatchState::from_parts_with_prefix` binds the prefix-relative geometry:
parsing starts at the prefix boundary with its sampling frontier there
(upstream sets `nextToUpdate` to the loaded dictionary end), and the existing
window floor bounds every emitted offset. A nonzero prefix now admits the
Greedy, Lazy and Lazy2 chain and row families beside Fast and DoubleFast; the
tree family (BinaryTreeLazy2) and every optimal-family profile still reject
combined staging with `UnsupportedFeature` until their own stages.

Thirteen focused commands pass: the complete library suite, true no_std
compilation and strict Clippy under safe, the isolated match-table and
dictionary backends the combined path exercises, and all features, plus
formatting. Every full suite reports 1120 passed
and 25 ignored across
1 result groups. Two new unit tests prove chain-family
(Greedy/Lazy/Lazy2) and row-family dictionary reach, output shrink against the
dictionary-free emission and roundtrip decoding with deterministic noise; the
unsupported-profile rejection test moved to the tree and optimal-family
levels.
Evidence: `evidence/2026-10-30-one-shot-encode-parameters-family.json`.
