# Bounded dictionary training

This design precedes the training portion of Module 7. It implements an
independent, deterministic trainer rather than translating the C implementation.
The pinned reference remains facebook/zstd v1.5.7. COVER/fastCover optimization,
shrink selection, and byte-identical upstream training are separate obligations;
content selection alone does not satisfy their test groups.

## Raw content selection

`RawTrainingOptions` specifies a dictionary byte cap, a sample byte budget, and a
segment size. Defaults are 8 KiB, 8 MiB, and 64 bytes. Dictionary capacity is at
least eight bytes and no larger than the sample budget. Segment sizes range from
eight through 4096 bytes. The sample budget is bounded by `u32::MAX`, allowing
checked u32 frequency counters. Invalid options fail before allocation.

The input is a borrowed slice of sample slices. The call retains no input beyond
its lifetime. A checked sum determines total sample bytes. When the budget is
smaller, each sample receives a proportional contiguous prefix, calculated with
u128 arithmetic; quotas round down and never exceed the aggregate budget. Samples
with fewer than eight selected bytes cannot contribute a candidate. Empty or
insufficient data reports a training failure instead of inventing content.

Each selected sample contributes overlapping candidate segments with half-segment
stride, including its final segment. Four-byte keys hash into a fixed 65,536-cell
frequency table. A segment's score is the sum of current key frequencies. A
max-heap stores candidate indices/ranges and score upper bounds. After choosing a
segment, its covered hash cells become zero; stale candidates are rescored when
popped and reinserted while another candidate has a higher bound. Scores only
fall, so lazy rescoring preserves greedy order. Ties prefer earlier sample/start
indices. Selection stops at capacity or when no uncovered keys remain.

The output is selected content in reverse selection order: the most useful first
segment appears at the dictionary's end, where shorter offsets can address it.
The final segment may be shortened to the remaining byte budget, but never below
eight bytes. Hash collisions affect selection quality, never bounds or format
validity. The algorithm does not depend on pointer identity or sample addresses.
Training quality is established by held-out sample compression against the
independent C codec and zstdoxide, with deterministic output and memory bounds.

## Data structures and allocation

- `SampleWindow` contains a sample index and selected length. It is a plain value
  record; the actual bytes remain in the caller's slices and are accessed through
  checked indices.
- `Candidate` contains a sample index, start/end indices, and a u64 score. Its
  ordering implements deterministic heap ties. Candidates and chosen segments
  are owned by fallibly reserved `Vec`/`BinaryHeap` allocations. No pointer or
  borrowed sample is stored in heap nodes.
- `TrainingFrequencies` owns exactly 65,536 initialized u32 counters. Construction
  and indexing are safe first. A separate `unsafe-training` feature will select
  equivalent unchecked cell access only after a fixed-width hash has established
  its index bound, with complete local initialization/lifetime/aliasing proofs.
- `Dictionary` owns the selected byte vector. The raw trainer publishes a normal
  raw dictionary and has no hidden C dependency. Large allocations use fallible
  reservation; these boundaries remain suitable for future allocator parameters.

Selected windows are bounded by budget/8, and candidate count is bounded by a
small constant times selected bytes/segment stride. Heap and selected vectors
reserve fallibly before every insertion. A score is at most 4093*u32::MAX and
fits u64; counters cannot exceed the checked selected-byte budget. Output length
never exceeds dictionary capacity. No comparison uses floating point.

## Formatted finalization

A separate finalizer will serialize magic, a caller-selected or deterministic
public-range ID, a valid literal Huffman tree, normalized OF/ML/LL tables, repeat
offsets, and bounded content. Entropy statistics and serialization must be
implemented and independently verified before exposing a formatted training API;
copying an opaque fixture header is not training. Common entropy-writing
foundations may precede the broader adaptive-encoder roadmap module because this
format requires them. The finalizer must accept custom content and samples,
reserve metadata space, preserve the most useful content suffix, and ensure at
least eight content bytes for initial `[1,4,8]` repeat offsets.

Before implementation, entropy construction and finalization will be decomposed
further in TODO.md. Independent loader/compressor/decompressor checks must verify
every produced dictionary. Missing upstream trainer strategies and parameter
matrices remain explicitly open in the test inventory.
