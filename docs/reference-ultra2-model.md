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
(53 requests). Complete normal/optimized recipe reconciliation and freezing are
tracked separately in TODO.md. Native instrumentation, its reviewed protocol and
all later Rust/public admission gates remain separate work items.
