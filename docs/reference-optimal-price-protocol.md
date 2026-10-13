# Bounded Optimal frequency and price observation protocol

This document specifies test-only source/operation requests before any C helper
or Rust price implementation. It targets `optLevel=0` in the pinned v1.5.7
`lib/compress/zstd_opt.c`, revision
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`. It does not enable strategy 7.
Dictionary-derived prices, fractional weights and optimal path selection are
outside this protocol. Derived constants retain upstream BSD attribution when
materialized into recipes or independent replay code.

## Transport and ownership

A request owns at most 131,072 source bytes and 128 operations. The source is
immutable; operations refer to checked offset/length ranges, never pointers or
implicitly padded tails. An exact binary packet has an eight-byte magic
`ZOPPR01\0`, little-endian u32 source length, operation count and literal mode,
then source bytes and exactly six little-endian u32 words per operation. Literal
mode 0 means automatic/compressed pricing, 1 means disabled literal compression.
The maximum packet is 134,164 bytes. Trailing bytes, unknown tags/modes,
truncation, integer narrowing and inconsistent counts are rejected before C.
Unused operation words must be zero. JSON recipe integers exclude bool and float.

Each process initializes one `optState_t` with independently owned, fully zeroed
literal[256], LL[36], ML[53] and OF[32] arrays and a zeroed entropy-cost record
whose Huffman repeat mode is not valid. The helper does not construct a CCtx,
search table, dictionary, optimal path, frame or predicted result. One request is
one isolated statistics lifecycle. All normal and instrumented builds must emit
identical complete records. Guarded backing storage and ASan/UBSan qualify helper
access; a successful scalar capture does not qualify production search safety.

## Operation words

| Tag | Remaining words | Meaning and checked domain |
| --- | --- | --- |
| 0: initialize | source offset, source length, 0, 0, 0 | Exactly once, first; calls rescale with zero LL sum |
| 1: update | literal offset, literal length, encoded offBase, match length, 0 | Existing initialized state; literal length 0..131,071; match length 3..131,072; nonzero offBase 1..u32::MAX; calls update without refreshing base prices |
| 2: refresh | 0, 0, 0, 0, 0 | Existing initialized state; refreshes all applicable base prices |
| 3: rescale | source offset, source length, 0, 0, 0 | Existing initialized state; rescale for next block and refresh; source range is explicit even though subsequent rescaling uses retained frequencies |
| 4: query | literal offset, literal length, LL price length, encoded offBase, match length | Existing initialized, refreshed state; raw literal range 0..131,072, independent LL length 0..131,072, nonzero offBase, match length 3..131,072 |

Every source range must fit the owned source without wrapping. Match length is
an abstract price input; it does not authorize reading a matching source region
or claim a valid compressed sequence. `offBase` is the stored numeric sum type,
not a physical distance. Its highest-bit index is at most 31. This independent
price domain intentionally includes encoded offsets larger than the proposed
public window cap, to qualify the long-offset penalty and scalar boundaries.

At most 32 update operations occur in one request, with total updated literal
bytes at most 4,194,304. Counts/sums and intermediate products are checked in
independent replay; no wraparound is accepted. A query after an update requires
an explicit refresh or rescale. Updates expose intentionally stale cached base
prices, but this protocol does not invoke price functions on that dirty state.
Positive raw-literal queries in dynamic compressed mode additionally require
literal sum at least one; empty raw-literal queries remain valid. This avoids
calling upstream's positive-literal path with an invalid zero base price after
empty initialization. Disabled literal compression leaves literal statistics
and their base price at initialized zero throughout the lifecycle.

## Complete observations and independent replay

After every operation emit its index/tag, the pricing mode, all 377 frequencies,
four sums and four cached base prices. Query records also include raw-literal,
LL and match prices; non-query records carry explicit null query results.
Initialization records distinguish predefined input sizes 0..8 from dynamic
size 9 and above. Subsequent rescale is dynamic even after a predefined first
block. No literal histogram, array entry or cached price may be omitted in favor
of a digest. A final request/source identity and exact operation count close the
record. Parsing is bounded and rejects duplicate keys, extra/missing fields,
wrong numeric types, malformed order and unconsumed records.

Independent replay begins with zero arrays. It builds literal histograms from
source ranges, applies explicit positive/zero bases when downscaling, and derives
LL/ML codes from the format's symbol intervals rather than calling the C helper.
It checks complete tables and sums after each transition. Base prices are
refreshed only by initialize/refresh/rescale. Whole-bit weight uses
`floor(log2(stat + 1)) * 256`; stats are bounded before adding one.

Dynamic raw literals have a minimum one-bit cost per literal. LL length 131,072
uses the pinned one-bit-more-than-131,071 special case. Dynamic match prices
include the long-offset penalty beginning at code 20 and the integer heuristic
`256 / 5 = 51`; predefined match pricing does not use these dynamic terms.
The independent scalar model must explicitly qualify these boundaries. Derived
base LL/OF distributions and extra-bit tables must carry source attribution.

## Required recipes and rejection controls

Freeze source-only cases for sizes 0/1/7/8/9, all byte values, absent symbols,
histogram counts around 255/256/257, and block sizes around 65,535/65,536/131,072.
Cover every LL/ML symbol interval edge, LL full-block special handling, offsets
around powers of two including codes 19/20/21/31, predefined-to-dynamic rescale,
statistics below/at/above the rescaling factors, disabled literals, grouped
updates before refresh, and repeated rescaling. Recipes do not contain predicted
observations, price outputs or C-derived admission decisions.

Before execution, reject malformed packets and schedules, invalid source ranges,
zero offsets, underlength matches, dirty-state queries, excess update budgets,
nonzero reserved fields and extra bytes under normal and optimized Python.
Review normal/instrumented helper compilation, all pinned dependency identities,
flags, environment and exact command inventory. After capture, independently
replay complete output and reject forged arrays, sums, stale/early-refreshed
prices, query values, command/build metadata and coherently edited records
without executing a child. Project into Rust fixtures only after both audits.

## Request transport checkpoint

The source-only encoder/parser round-trips nine boundary schedules and rejects
38 malformed requests in both normal and optimized Python, with zero child
attempts. Exact source and logs are recorded in
`evidence/2026-10-13-optimal-price-protocol.json` and its archive. This closes the
request boundary only: independent observation replay is specified above but
not implemented yet, and no C execution or price results are claimed.

## Version 2: bounded repeated updates

Recipe design exposed a coverage limit in version 1: 32 updates cannot raise
LL/ML/OF frequency sums to their rescaling threshold. Version 1 and its evidence
remain unchanged. The current price protocol is `ZOPPR02\0`, implemented in
`optimal_prices_v2.py`; it adds tag 5 with words `literal offset, literal length,
encoded offBase, match length, repeat count`. The helper invokes the same upstream
update primitive exactly that many times before emitting the complete state.
It does not synthesize a scaled update or skip validation. Each repeated update
has the same source slice and abstract sequence symbols.

Repeat count is 1..8,192. Across ordinary and repeated updates, at most 8,192
primitive calls and 4,194,304 updated literal bytes are allowed per request.
The 128-operation, source, packet, refresh and query bounds are unchanged.
Repeated updates mark cached prices dirty exactly like an ordinary update. Full
array/sum/cache observations are recorded after the batch, and independent replay
checks each arithmetic increment before accepting its final record. This reaches
all sequence-table rescaling branches with zero-length literal updates while
retaining bounded request/output sizes. It does not add arbitrary seeded state.

Normal and optimized version-2 request checks each pass ten exact round trips
and reject 42 malformed schedules/packets with zero child attempts. Version-1
controls still execute against the frozen version-1 module. No C capture has yet
been run for either protocol.

## Independent model checkpoint

The checked Python model replays all 147 source-only requests / 2,018 operations
with complete tables, sums, explicit cache refreshes and scalar prices. Hand-
checked weights, initial distributions, predefined/disabled literal costs, the
full-block LL special case, long-offset match penalty and downscale identities
pass. Normal and optimized checks each reject 23 altered observation transcripts
without child execution. `evidence/2026-10-13-optimal-price-model.json` retains
the exact model, inputs and logs. These are model controls; C observations are
not yet captured and are a separate gate.
