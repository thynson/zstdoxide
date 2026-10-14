# Captured broader Optimal search maps

Reconstruct with `python3 scripts/pack_optimal_match_broad_fixtures.py`. The
projector audits the immutable C capture and independent model before projection.
There are 37 cases, 82 operations and 2,119,568 source bytes.

The pack starts with `ZOMBFX2` plus a zero byte, then a little-endian u32 case
count. Each case has ten u32 words: name/source/query lengths, then window, hash,
chain, search, minimum, target and short-hash logs/values. Name and source follow.
Each query has six request words, before and after snapshots, and a candidate
count followed by offBase/length pairs. Each snapshot begins with two frontiers
and three sparse maps. A map stores its complete cell count, nonzero count and
strictly increasing index/nonzero-value pairs. All absent cells are zero.

The Rust reader independently rejects noncanonical maps and invalid bounds,
replays every candidate against source bytes and verifies complete continuity.
The original complete C maps remain in the capture archive; this representation
changes only storage, not observations. Public Optimal is not enabled.
