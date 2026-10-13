# Frozen broader lazy-family inputs and observations

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
was reached. The independently captured sequence/table/frontier observations below now
qualify final publication outcomes; no chronological probe trace is inferred.

Run `python3 scripts/check_lazy_family_recipes.py --check` (also under `-O`).
The checker verifies pinned code, predecessor recipes and observed fresh salt
at each depth; reconstructs the manifest and every packet; checks exact placed
byte extents, seed coordinates and arithmetic; and rejects 55 malformed packets.
It never starts a child process. The input manifest remains unchanged from before C execution.


The permanent `capture.json` and `observations.ndjson` contain 645 complete
direct observation groups plus six fresh-context salt groups. Each group has
an exact consecutive byte extent and SHA256. All direct bytes match both normal
and instrumented v1.5.7 builds. The pack is 16,705,002 bytes; its SHA256 is
`7979c382d9d613052a20378cea2563d397e4a1a0e146ea17acc7b1edfbb1416b`.
The 184,949-byte manifest SHA256 is
`153efc6849613cb6815d0d0980411da5eb0286ed8f5378cca7479f4e5c01a4de`.

Run `python3 scripts/check_lazy_family_fixture.py --check` (also under `-O`).
It independently replays complete tables and sequences, checks all 138 exact
tiny observations and 173 depth-zero Greedy controls, and reconciles 96 score
cases against their pre-frozen source placements. Thirty-six applicable threshold
cases retain the baseline below/equal and publish the candidate strictly above.
Thirty are inactive-depth controls, six observe Greedy's early repeat publication,
eighteen publish baseline-family repeats and six compare first/fourth successive
copies. Corresponding chain/row outputs agree. Final published repeats do not
by themselves prove initial selection; complete final maps are not a probe trace.

Capture source/compiler/auditor identities, final score observations and the
independent audit are retained in the manifest. See
`docs/reference-lazy-validation.md` for exact coverage, provenance and limitations.
No Rust algorithm generated these expectations. Rust projection and parsing,
ordinary/public frames and performance remain separate work items.
