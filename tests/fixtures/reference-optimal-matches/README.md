# Captured tiny Optimal search maps

Reconstruct with `python3 scripts/pack_optimal_match_fixtures.py`. The projector
verifies the immutable capture archive and independently replays every observation
before emitting exact C values. The 29 cases contain 273 queries. Both C builds
and both arena padding passes agree; one copy is stored after comparison.

The binary starts with ZOMFX01 followed by a zero byte and a u32 case count. Each
case has ten little-endian u32 words: name/source/query lengths followed by seven
geometry words (window, primary hash, chain, search, minimum, target, short hash).
ASCII name and source bytes follow. Each query has six request words, complete
before and after snapshots (two frontiers, all primary/edge/short-key cells),
then candidate count and offBase/length pairs. Cell extents derive from checked
geometry. The independent Rust reader bounds allocations and replays source bytes.

The original model comparison failed because primary minMatch3 hashing uses
hash4, not hash3. The archive preserves that failure; an additive v2 model passes
all complete observations. See docs/reference-optimal-validation.md.
