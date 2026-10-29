# Parameterized dictionary-backed one-shot compression validation

Module 36 is in progress. Only the internal combined-staging engine is
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
