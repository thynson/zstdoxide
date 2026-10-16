# Wider retained fixture design

The additive `ZORFW02\0` envelope keeps the checked `ZORFX01` integer field order
and chronological per-block publication records. It admits exactly the newly
qualified wider geometry: source <=16,384, two-to-four 64..8,192-byte blocks,
W10..16/H6..12/C6..12/S1..6, source inside its window, 4,099 path cells, <=32,768
events per block, <=73,730 flattened search cells and <=32,768 complete frame
bytes. The complete projected pack remains bounded by 16 MiB. The old fixture,
reader and scope remain immutable predecessor evidence.

The projection starts from the hash-verified complete capture archive and runs
the independent sparse model against both native transcripts before writing.
It preserves native canonical path/search deltas directly after checking their
extent/order/change invariants against reconstructed baselines. Frequencies use
canonical deltas computed from complete native records; per-block sequences
remain append-only. All baselines persist across blocks. Before-query sparse
changes must be empty against the prior complete state. All four native copies
of each complete frame and allocation ledger pair must reconcile.

The Rust reader borrows source/frame/name bytes and fallibly owns bounded event
and delta vectors. It independently checks representation, current-prefix source
matches, path reconstruction, exact frequency sums, schedule/frame extents, the
strict 384/192 adjustment and pending/committed owner transitions. All complete
C frames must decode to source. Negative controls include oversized fields,
noncanonical/unchanged deltas, out-of-range horizon cells, changed publication,
truncation and extra bytes. The horizon recipe must retain the observed 4,096
cell and 4,195 last coordinate; long-match recipes exceed the path horizon while
using the separately bounded sufficient-length exit. Only after these checks
may private retained production accept the wider domain.
