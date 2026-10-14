# Wider scripted Optimal path boundary plan

The first scripted path capture freezes complete initialized cell arrays for
sources up to 512 bytes. Keep it immutable. A separate ZOPHW02 protocol extends
source capacity to 8,192 bytes and caps the observed path owner at exactly
`min(source_length + 3, 4099)` cells. The copied upstream parser and its fifteen
instrumentation anchors remain unchanged. Match discovery remains scripted;
ordinary CCtx startup and retained block search are still unqualified.

The wider observer encodes each event's complete path state by a canonical delta
from the previous event. The initial state is all zero at each padding pass.
Every changed cell is listed once in ascending index order with all seven fields;
unchanged cells must not be listed. A changed cell may become zero. The declared
cell count is fixed for the request. Applying all deltas reconstructs every cell
at every event without losing stale fields or reinterpreting forward/reverse
phases. Both independent replay and C observer own a separate prior-state copy.
Only that copy changes when emitting a delta; parser storage is never modified
by observation. Guard and untouched-tail checks remain mandatory.

The independent wider replay reuses the already captured small-path transition
model. It changes only request bounds, bounded storage initialization, event
serialization and branch labels for the now-reachable 4096 horizon. Compare its
reconstructed complete states against every original small recipe before C
execution. The C adapter changes only source/packet/event capacities and snapshot
serialization; retain an explicit diff and original source digest for review.

Source-only recipes must distinguish initial lengths 4094/4095/4096, strict
sufficient thresholds, later paths ending at relative positions 4095/4096/4097,
source-end exits and maximum 8192-byte input. Branch coverage must prove that
near-horizon candidates were actually queried, rather than merely present in an
unused script. The model must reject path indices beyond 4098, event overflow,
malformed deltas, duplicate/unchanged cells and wrong baseline/pass ordering.

The event cap is 16,384 per padding pass, with a 128 MiB per-command output cap
and an explicitly bounded aggregate campaign. Scripts retain at most 64 positions
and eight candidates each. Source and literal owners have the same 64-byte prefix
and trailing allowance as the small observer; all arithmetic and comparisons are
checked before access. Static storage remains below 4 MiB. Large immediate matches
may exceed the path horizon without indexing a path cell at their length; the
reconstruction destination remains determined by the predecessor position.

This gate qualifies the bounded scripted path boundary, not a production parser.
A private Rust path implementation still requires its own ownership, phase,
transactional failure, allocator and complete state comparisons before public
admission can be considered.
