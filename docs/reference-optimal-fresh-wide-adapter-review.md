# Complete fresh Optimal adapter review

The generated adapter diff is against the frozen wider retained observer.
The only admission changes are the distinct magic, exactly one block, minimum
source length 64, maximum block length 16,384, packet extent 16,432, and
65,536 events / 5,462 sequence records. The fourteen uniquely anchored upstream
observation substitutions are identical. No parser/provider is replaced.
The ordinary CCtx receives one complete-source Continue and an empty End.

All 4,099 path cells are compared by their seven named fields. Both 73,730-cell
search snapshot arrays cover all admitted heads, edges, optional short heads
and frontiers. Pre-query state is compared before advancing the post-query
baseline. Static source/decoded storage is 16,384 bytes, frame capacity 32,768;
source padding and 64-byte guards remain initialized and checked. Every scalar
and packet extent is admitted before copying, shifting or native allocation.
The source fits its window and all positions remain inside that allocation.

The unchanged zero-initializing guarded CCtx allocator checks alignment,
32-allocation/8-MiB bounds, exact owner frees and upstream sanitizer redzones.
Both padding values observe all events and both repeat-state owners, with
raw rollback distinguished from compressed publication. Complete frames are
independently decoded by the pinned reference; Rust decoding follows fixture
projection. Syntax/preflight checks execute no native codec operation. Frozen
capture input files must remain byte-identical after native execution begins.
