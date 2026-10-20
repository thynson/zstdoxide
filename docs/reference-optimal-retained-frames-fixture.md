# Larger retained Optimal observation fixture

The proposed additive pack `ZORFS03\0` retains the immutable wide fixture's
little-endian schema: bounded case headers, borrowed source/frame/name bytes,
normal and sanitized allocator summaries, complete block publication records,
and persistent sparse path/frequency/search deltas. Its maximum is 16 MiB,
with one to nineteen cases, total source length 128..32768 per case, two to four
64..8192-byte blocks and complete frames of at most 32832 bytes. It does not
change or replace the prior `ZORFW02` pack.

The projection consumes only a completed independently audited native archive.
It verifies every archive member, request, captured source identity, both native
build transcripts, all four frames and the source-derived independent model
before projecting the first padding pass. The nineteen source-only hypotheses
predict 67321 projected events. Counts must agree with actual capture; no missing
or rejected native case may be dropped to satisfy that prediction.

The independent Rust reader owns bounded fallible observation vectors and borrows
name/source/frame slices from the immutable pack. It checks complete array extents
before allocations, sorted nonredundant deltas, all 4099 path cells, 386 frequency
words, search owners/frontiers, exact block-local source footprints, sequence
repeat intent and chronological publication owners. Cross-block matches may
read consumed prefix bytes; future backing bytes remain unavailable through the
checked prefix. Raw/RLE retains the previous compressed owner and compressed
publication swaps it. Complete frame structure must agree with publication.
The reader supplies no parser matches or candidate transitions to production.

Reader qualification must cover all native complete frames through independent
Rust decompression, source/schedule boundaries, later-block publication, horizon
cell 4096, short-key geometry, strict frame prefixes, truncation, extra bytes,
overlarge counts and malformed deltas. Only after the archive, projection and
reader gates pass may the private retained parser/search source ceiling widen.
Public frame admission remains a later separate milestone.
