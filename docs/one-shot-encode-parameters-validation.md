# Parameterized dictionary-backed one-shot compression validation

Module 36 is in progress. The internal combined-staging engine, its DoubleFast
extension, the Greedy chain-and-row family extension, the BinaryTreeLazy2
tree-family extension with the explicit optimal-family rejection, prepared
formatted-dictionary entropy reuse, and the public allocating, caller-owned and
coordinator parameterized dictionary calls are qualified and integrated.
Corpus validation, the complete feature/release regression, measurements and
reconciliation remain open.

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

## Combined staging extension to BinaryTreeLazy2 and explicit optimal rejection

The tree family needs no seeding pass: `GreedyMatchState::
from_parts_with_prefix` keeps its sampling frontier at the combined-buffer
start, so the first tree query back-fills the whole staged region through the
unsorted-insert frontier exactly as the plain path back-fills its bounded gap.
A nonzero prefix now admits the complete Greedy family (chain, row and tree)
beside Fast and DoubleFast. Under the design's supported-subset clause every
optimal-family profile - fresh and retained Optimal, Ultra and Ultra2 - is
explicitly rejected with `UnsupportedFeature` instead of silently dropping the
dictionary.

Thirteen focused commands pass: the complete library suite, true no_std
compilation and strict Clippy under safe, the isolated match-table and
dictionary backends the combined path exercises, and all features, plus
formatting. Every full suite reports 1122 passed
and 25 ignored across
1 result groups. Two new unit tests cover tree-family dictionary reach,
output shrink against the dictionary-free emission and roundtrip decoding at
level 9, and the explicit rejection of the retained Optimal/Ultra/Ultra2
plans on capped-table requests; the fresh optimal-family rejection test keeps
covering levels 12/14/17.
Evidence: `evidence/2026-10-30-one-shot-encode-parameters-tree.json`.

## Prepared formatted-dictionary entropy reuse

A formatted dictionary's validated literal codebook, sequence tables and
repeat triple seed the first emitted block through the existing transactional
entropy owner, matching `ZSTD_loadCEntropy` filling `blockState.prevCBlock`
before the first block. Covered literals may therefore use a treeless header
and covered sequences may repeat the prepared tables; a block containing a
symbol absent from the prepared tree still selects a fresh tree, and raw or
empty dictionaries keep their exact existing bytes. The seed is borrowed only
during workspace construction and copies no dictionary ownership into the
frame.

Sixteen focused commands pass: the complete library suite, true no_std
compilation and strict Clippy under safe, the isolated huffman-codebook,
FSE-encoder and dictionary backends the new path exercises, and all features,
plus formatting. Every full suite reports 1124 passed
and 25 ignored across
1 result groups. Two new unit tests prove treeless literal and repeat
sequence reuse across the Fast, Greedy and BinaryTreeLazy2 tiers with the
formatted fixture, exact shrink against the same raw dictionary content, and
that a literal block containing a symbol absent from the prepared tree never
emits a treeless header.
Evidence: `evidence/2026-10-30-one-shot-encode-parameters-entropy.json`.

## Public allocating, caller-owned and coordinator calls

`compress_with_parameters_and_dictionary(_in)` resolves a request against the
source and the dictionary's matchable content length, then emits the admitted
combined-staging profile. `compress_into_with_parameters(_in)` and
`compress_into_with_parameters_and_dictionary(_in)` write complete headers and
blocks atomically into caller storage and return the exact written length;
short output reports `ResourceLimit` after earlier whole appends without
touching guard bytes. `OneShotEncoder::compress_into_with_parameters(_in)` keeps
requests call-scoped, claims pending prefixes before resolution, preserves
persistent selection across success and failure, and clones no handle. A
nonzero dictionary that resolves to Optimal/Ultra/Ultra2 is rejected with
`UnsupportedFeature` before the first output byte instead of dropping the
dictionary; empty and raw attachments preserve the dictionary-free
parameterized bytes.

Twenty-three focused commands pass: the complete library suite, true no_std
compilation, strict Clippy and the public integration suite under safe, the
isolated frame-sink, huffman-codebook and dictionary backends, and all
features, plus safe/all doctests and formatting. Every library suite reports
1124 passed and 25 ignored across 1 result groups. Six public
integration tests cover exact caller-owned byte equality and guards, formatted
and raw dictionary frames with header flags, unsupported-profile rejection
before writes, coordinator persistence and prefix consumption, and every
observed allocation failure for allocating and caller-owned entry points with
independent owner release. Doctests establish dictionary-view and input/output
non-overlap; the usage guide records the supported subset and limits.
Evidence: `evidence/2026-10-30-one-shot-encode-parameters-public-api.json`.

## Guarded native parameterized schedules

Twenty typed parameterized dictionary cases run against normal and sanitized
builds of the pinned v1.5.7 library: Fast, DoubleFast, Greedy, Lazy, Lazy2 and
BinaryTreeLazy2 with raw and prepared attachments, empty input, prepared
entropy, window-bounded inputs, multiblock sources, and five fresh/retained
Optimal/Ultra/Ultra2 profiles that Rust rejects before writing. Every native row
uses independent 16-byte destination guards; both builds produce the same
twenty rows and twenty native frames. Rust frames decode natively and native
frames decode in Rust for every case, and each observable dictionary control
differs from its fresh no-dictionary control. Native compressed-byte,
capacity-precedence and workspace identity are not Rust contracts; the
window-bounded cases establish interoperation and Rust-side suffix expiry is
covered by the focused unit campaign rather than a native byte-equality claim.

Ten commands pass: Rust input export, normal and sanitized oracle builds and
captures, safe and all-feature replays and strict Clippy, and formatting.
Evidence: `evidence/2026-10-30-one-shot-encode-parameters-native.json`.

## Concrete no-atomics ARM emission

The parameterized dictionary allocating, caller-owned and coordinator paths run
on the host under safe and all features with exact guards and prefix
consumption. Concrete `thumbv6m-none-eabi` library checks and release LLVM IR /
object emission pass for both safe and every opt-in storage backend together;
the retained artifacts include the emitted modules. Foreign Miri was not run
because this module added and modified no unsafe code, per the standing
direction. Native byte identity and complete foreign-platform behavior are not
claimed.

Nine focused commands pass: two host test suites, two strict Clippy checks,
two concrete atomic-free checks, two IR/object emissions and formatting.
Evidence: `evidence/2026-10-30-one-shot-encode-parameters-platform.json`.
