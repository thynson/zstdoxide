# Frozen broader lazy-family inputs

These 645 inputs (303 chain, 342 row; 215 per depth) contain 18,900,771 source
bytes and 19,019,595 packet bytes. The collection preserves all 138 tiny Lazy
inputs and expands all 137 broader Greedy inputs at depths zero, one and two.
The 173 depth-zero predecessors have an explicit compatibility map.

The inherited inputs cover effective minimum widths, independent table layouts,
row widths and tag/head wrap, capped budgets, collisions, cyclic history,
long pending gaps, incompressible skipping, prefix/window floors, backward
extension, repeat periods, 16-bit length transitions and the maximum admitted
history. Their existing observations do not supply expected Lazy results.

Ninety-six additional inputs place disjoint known byte prefixes in 4,096 bytes
of history, followed by a 96-byte unique source sequence. Source sentinels stop
matches and backward extension at known byte boundaries. Seed coordinates and
repeat distances are explicit. OffBase values are exact powers of two for
closed-form high-bit costs. At both lookahead depths, placed explicit and repeat
copies fall below, equal to and above the corresponding gain threshold. Other
inputs put baseline table/repeat lengths below, equal and above each other, or
place four progressively longer explicit copies at consecutive queries.

`score_hypotheses` records arithmetic over placed byte copies. It is not an
expected match, a parser implementation or evidence that an intended branch
was reached. Actual sequence/table/frontier observations must establish coverage
in the future C campaign; unsuccessful hypotheses remain preserved.

Run `python3 scripts/check_lazy_family_recipes.py --check` (also under `-O`).
The checker verifies pinned code, predecessor recipes and observed fresh salt
at each depth; reconstructs the manifest and every packet; checks exact placed
byte extents, seed coordinates and arithmetic; and rejects 55 malformed packets.
It never starts a child process. No broader Lazy C result or Rust-generated
expected output is present in this input collection.
