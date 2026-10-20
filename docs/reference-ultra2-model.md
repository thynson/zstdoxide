# Bounded Ultra2 independent model qualification

Public strategy 9 remains unsupported. This module first qualifies its private
initial discarded pass and history-coordinate lifecycle against pinned
facebook/zstd v1.5.7, revision `f8745da6ff1ad1e7bab384bd1f9d742439278e99`.
No existing Ultra or Optimal input, fixture, frame or owner baseline is rewritten.

## Eligibility and coordinates

`oracles/ultra2_lifecycle.py` independently models all five native predicates:
zero literal-length sum, empty sequence store, equal dictionary/low limits,
current position at the dictionary limit and source length strictly above eight.
All 32 combinations are checked. The helper's unsigned update-frontier assertion
is checked separately; it must not become an extra skip predicate. The model
rejects overflow before rebasing a complete integer interval.

Copied coordinates translate only current-source reads. All old source bytes
become stale after rebasing, including indices sharing physical tree ring slots
with the new interval. Empty ranges may name the end; positive reads may not.
Controls cover 7/8/9, source and u32 limits, malformed integer types, complete old
interval exclusion, forward/self candidates and chain-log 6..12 ring aliases.
Both normal and optimized Python pass 120 rejection controls without children.

A sequence-store-only skip hypothesis preserves the otherwise fresh parser
state. Its appended ordinary observations reconcile every state of all 22
unchanged Ultra native transcripts: 130901 logical events, with exact request
and normal/sanitized transcript identities. This does not claim that an opaque
preexisting sequence payload is encoded, that other nonfresh states are modeled,
or that a native Ultra2 wrapper has already executed. Other failed predicates
are independently checked eligibility arithmetic at this stage.

The source-bound qualification retains both command logs, complete source
snapshots and identical before/after hashes. Native ordinary inputs are pinned
by the immutable existing fresh-capture archive and fixture provenance; they
are not duplicated or relabeled as Ultra2 observations. The archived report is
`evidence/2026-10-20-ultra2-lifecycle.json` and its adjacent reconstructible archive.

## Two-pass source predictions

The separate model retains physical hash/tree/short-key cells and frequency/path
owners between passes. Its checked source view exposes no readable stale prefix.
Each invocation restarts path-local scratch and original repeats; discarded
output lengths reset without a tail statistics update. These rules reuse the
already qualified ordinary search/path/frequency transitions, with explicit
integer coordinate translation rather than pointer rebasing.

The source-only corpus is preselected: all 22 preceding Ultra sources in both
seeded and skipped forms, three threshold controls and six tree-ring residues
(53 requests). All 53 requests have now passed complete normal/optimized model reconciliation:
394656 logical events, including full first-pass equality against the preceding
native ordinary transcripts for all 44 predecessor variants. The two changed
final paths are `u2_seed_alternative_seed_0` and `u2_seed_alternative_seed_2`.
Six ring cases exercise 28 queries at stale physical tree slots and 378 queries
with stale short-key cells. Those cells are retained physically and excluded
from readable history by checked coordinates.

The optimized qualification and normal complete-wire campaign retain source
snapshots and before/after hashes. The separate sequential normal survey agrees
on every complete model-row hash; it was not itself started with a contemporaneous
source snapshot, so the source-bound campaigns supply that provenance. Each
model worker prohibits child execution. Python worker concurrency is not native
codec execution or a timing comparison.

The sealed manifest is `tests/fixtures/reference-ultra2-recipes/recipes.json`,
SHA-256 `05025827219df1b3c4c1c4d752bea23f3a05e54814e0143a661ab17edc51e16a`.
Every canonical wire record is independently computed before C execution. The
complete predictions total 1163199991 bytes per native build; the largest single
transcript is 216085829 bytes, below the fixed 256 MiB cap. All per-line and
campaign bounds pass. Literal manifest hashing binds these full-state predictions
in the native capture specification, so later coherent output changes cannot be
accepted by simply updating capture metadata.

The source-only qualification archive is
`evidence/2026-10-20-ultra2-recipes.tar.gz`, SHA-256
`24e3b3cd44f5af20279345c70dbe6330f4f8610acabac3c488e9709a57542834`.
Its 286 retained files include all workers, normal/optimized results, drivers and
the frozen manifest; independent archive reconstruction checks every file.
Native observation, its protocol controls and all Rust/public admission gates
remain separate work items.
