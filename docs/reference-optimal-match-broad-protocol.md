# Broader direct Optimal match observation protocol

This additive protocol preserves every tiny ZOPMT01 packet and its reconciled
capture. ZOPMB02 followed by a zero byte uses the same nine header words and six
query words, but admits source lengths 8..524,288, 1..16 queries and window logs
10..21. Hash/chain logs remain 6..12 and search logs 1..8. Minimum is 3..7 and
applied target is 0..131,072. Packet size is exactly 44 + source + 24 * queries,
at most 524,716 bytes. All scalar type, range, ordering and exact-EOF checks apply.

For minimum three, short-key log is exactly min(window log,17), matching the
pinned CCtx allocation rule. For other minima it is zero. The primary hash uses
four bytes for minimum three/four, five for minimum five and six for six/seven.
The separate short-key owner uses hash3. This distinction corrects the original
tiny-model error; original evidence is retained and qualified separately.

Thirty-seven source-only recipes total 2,119,568 bytes and 82 queries. Distances
262,143/262,144/262,145 under windows18/19 separate the inclusive window bound
from the short-key search's strict distance <2^18 heuristic. Requests also cover
ordinary short-key logs across windows10/14/17/18/19/21, absence for minima4..7,
repeat thresholds0/3/4/7/8/11/12/4095/4096 with both literal-zero modes, and maximum
source/query extents. These are branch hypotheses, not observed match results.

Native prefix bias remains two. Repeats stay <=min(4096,query+2), keeping even
unused pointer expressions in the owned arena. Fresh repeat values1/4/8 are
observed only at query positions>=6. This does not claim equivalence to ordinary
parser startup at positions0/1, whose integration is a later retained-state gate.
All queries leave eight source bytes, and length-to-beat cannot exceed remaining
source. No arbitrary table seeds, external dictionary or wrapped source span is
admitted. Both update frontiers and all owner maps remain explicit observations.

The wider replay uses checked 4,096-byte equal-slice chunks before examining the
first unequal chunk bytewise. This avoids quadratic Python byte iteration on
long equal suffixes while retaining an independent source-byte comparison; it
does not reuse C's word loads, common-prefix state or tree pointer traversal.
Its scalar predecessor must agree on the entire tiny input set and deterministic
small-source comparisons before wider C execution. Models are never result
fixtures; only actual C snapshots may be projected into Rust tests.

The prospective helper owns primary/edge capacities4096 and short-key capacity
131072, each guarded and explicitly initialized. Main and short frontiers start
at two, before/after snapshots are complete, and candidate capacity4099 remains
bounded by the search budget. Source, packet, owners and snapshot copies must fit
a static4MiB helper-controlled budget. Observe two padding passes and fully
instrument every used codec unit. Exact output/time bounds, independent replay
and forged evidence controls must be reviewed before execution. This protocol
does not enable public Optimal or admit larger production hash/chain geometry.
