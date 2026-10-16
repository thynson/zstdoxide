# Wider sparse retained observer review

The additive helper preserves the fourteen unchanged-provider observation hooks
from the qualified startup/retained helpers. The generated diff is against the
small retained helper, separately from the exact pinned upstream transformation.
No native provider, parser transition or entropy decision is replaced. Ordinary
Begin/Continue/End calls and both live compressed-state owners remain observed.

Source/decoded storage is fixed at 16,384 bytes, frame storage at 32,768, sequence
records at 2,731, path state at exactly `ZSTD_OPT_SIZE` (4,099), and per-block events
at 32,768. Packet admission checks every scalar before shifts/copies, complete
source/schedule extents, and source length <= selected window. Heads/edges are
bounded by 4,096 raw cells each; short heads by 65,536. The two flattened snapshot
arrays each have 73,730 initialized u32 cells. These are observer-owned static
arrays, independent of the two guarded CCtx allocator owners. Prefix pointers
remain within the immutable contiguous source and guarded padding allocation.

Path deltas compare all seven named initialized fields, avoiding struct-padding
comparisons. Search snapshots copy exactly the admitted head/edge/short extents
and two frontiers, after native owner-extent checks. Pre-query deltas are emitted
against the preceding complete baseline before post-query deltas advance it.
Baselines clear only before a new padding pass, preserving stale cells across
blocks. Full frequency tables, query candidates and sequence observations remain
unchanged. The path-tail-zero field is vacuous outside the now-complete path
owner; all 4,099 actual cells are represented, including unchanged zero cells.

The existing zero-initializing allocator retains alignment, 64-byte guards,
32-allocation/8-MiB bounds and exact free identity checks. Upstream sanitizer
workspace redzones and pointer-overflow annotations remain unchanged. The wider
source is still entirely inside one window, so no pointer/history expiry claim
is made. Complete source and guards are checked at every observer event; final
frames decode through the pinned oracle. Independent Rust decoding is a later
gate. No timing claim follows from capture execution.
