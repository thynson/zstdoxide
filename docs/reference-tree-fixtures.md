# Direct binary-tree fixture projection

This is a test-only representation of the audited v1 C observations. It adds no
codec storage or public support. The source is the immutable broader capture
archive, including all thirty tiny predecessor controls. No archived code runs.

The `ZTREEFX1` binary uses little-endian u32 integers throughout. Its header is
an eight-byte magic and case count (103). Each case begins with name byte length,
source byte length, operation count, and the seven original parameter integers
(W, C, H, S, M, target, strategy), followed by ASCII name and source bytes.
Each operation stores query, before frontier, after frontier, raw window floor,
raw tree floor, selected match length and offset base, then all 2^H hash heads
and 2^C edge cells in original paired-node order. Values retain the upstream
bias and zero/one sentinels. No native Rust layout or pointer is serialized.

Only normal-build first-padding operation records are projected. The original
capture auditor proves equality to the other padding and sanitizer records.
Initial before-maps must be all zero, and every later before-map must exactly
match the preceding after-map; this permits omission of redundant before-maps
without losing their reconstruction. Frontier scalars remain explicit. Source
and guard invariants and the original completion ledger remain in the immutable
archive and are checked before projection, rather than fabricated as observations
in the smaller binary. Each case's complete stdout and packet hashes are recorded
in provenance alongside the archive and projector identities.

Projection must require the frozen capture's exact archive identity, bounded
regular members and complete chunk/file identities. It independently compares
all source bytes and scheduled queries with the frozen input recipes. It checks
map continuity before writing and refuses an existing output. A separate
validation mode compares a supplied binary with a fresh projection, without
rewriting either. Checked readers must bound each dimension, checked-multiply
map extents, reject truncation and trailing bytes, and replay selected source
matches. The next work item is an independent dependency-free Rust test reader;
projection alone is not evidence that a Rust tree algorithm matches C.
