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

## Entropy-writing and formatted-finalization checkpoints

The first finalizer learns a full-alphabet literal prefix tree from smoothed
sample-byte frequencies and serializes the existing attributed predefined
OF/ML/LL distributions as sequence priors. Adaptive sequence statistics belong
to the following entropy-encoder module; this baseline must not be described as
upstream ZDICT entropy optimization. It creates its metadata algorithmically,
without copying an opaque dictionary fixture.

1. A fallible forward bit writer serializes normalized-count descriptions by
   inverting the checked format reader's cutoff/threshold and zero-run rules.
   Accept table logs 5..=12, counts >=-1, nonempty alphabet at most256 symbols,
   and exact absolute mass2^log. Test byte equality against the pinned C header
   vectors, including single-symbol prepared-table headers and long zero runs.
2. Frequency normalization uses one unit for every positive symbol and allocates
   remaining mass proportionally with integer largest remainders. Ties prefer
   lower symbols. Counts with unit mass may use the format's -1 marker. This is
   deterministic normalization, not a claim of reproducing FSE_normalizeCount.
3. A two-state FSE writer encodes explicit Huffman weights. It reuses owned
   FseTable and its existing independently gated safe/unsafe access backend.
   Invert transitions with bounded checked scans initially; no new pointer or
   unchecked inverse-table structure is necessary. Choose terminal states whose
   update needs positive bits, serialize preceding transitions in reverse, then
   initial states and the stop marker. Verify all outputs with independent C
   FSE decoding and all relevant Rust termination/bounds checks. A plain forward
   bit buffer and normalized integer records require no artificial unsafe code.
4. Literal tree construction owns a fixed alphabet frequency/length/code record
   and indexed tree/heap nodes. Construct an ordinary deterministic Huffman tree;
   if its depth exceeds11, use a bounded weighted partition whose child leaf
   counts fit the remaining depth. Preserve a complete prefix tree and all256
   symbols with pseudocount1. Convert lengths to canonical weights. A flat8-bit
   tree is replaced by a valid near-flat tree (one7-bit,2538-bit,two9-bit leaves)
   so its255 explicit weights have at least two symbols and can be FSE encoded.
   Select the highest-frequency symbol for the shorter code and the two least
   frequent for longer codes. Serialization must fit the127-byte weight payload;
   a deterministic near-flat fallback remains available when a tree's metadata
   would exceed this format limit. Every fallback is built from explicit lengths
   and validated, not an imported byte header.
5. `DictionaryTrainingOptions` combines raw selection options and an optional
   nonzero caller ID. `train_dictionary` selects content then finalizes it;
   `finalize_dictionary` accepts custom content and training samples. The final
   byte cap is the selection's dictionary_size and must be at least256. Auto IDs
   are deterministic XXH64-derived values in32768..2^31, exclusive of2^31.
   The finalizer writes magic, ID, learned literal tree, sequence priors,
   `[1,4,8]` repeat offsets and content. Preserve the most useful content suffix
   when metadata consumes capacity, and zero-pad before short content to8bytes.
   Return a normal parsed Dictionary so all loader invariants are revalidated.
6. Test dictionary metadata with pinned C loading, C compression/Rust decoding,
   Rust compression/C decoding, custom IDs, tiny/corrupt/overflow parameters,
   held-out effectiveness, resets, and applicable official training assertions.
   Report COVER/fastCover, shrink optimization and strategy-specific original
   matrices separately until they are implemented.

`EntropyBits`, frequency arrays, normalized arrays and node records are setup
values owned by their containing Vec/array; borrowed source bytes never enter
persistent state. All dynamic reservations are fallible. Existing FSE tables
retain their safe/unsafe feature boundary. If a later hot-path encoder adds a
new unchecked inverse/code table, that structure needs its own opt-in feature
and benchmark before it is enabled.
