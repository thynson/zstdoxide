# Direct binary-tree oracle contract

This contract precedes its helper and any C execution. Reference source is
facebook/zstd v1.5.7 revision `f8745da6ff1ad1e7bab384bd1f9d742439278e99`,
under the attribution in `LICENSE.zstd`. BinaryTreeLazy2 is still unsupported.
Existing Lazy/Greedy helpers, protocols, scripts, observations and baselines are
immutable. The first tree helper observes search state only, not complete frames
or a claimed full parser trajectory.

## Requests and independent owners

Use a separate `ZTREE01\0` little-endian packet. A fixed 80-byte header contains:

| Offset | Bytes | Field |
| --- | ---: | --- |
| 0 | 8 | Magic |
| 8 | 4 | Version, exactly 1 |
| 12 | 4 | Header bytes, exactly 80 |
| 16 | 4 | Source bytes N, 8..131072 |
| 20 | 4 | Operation count K, 1..256 |
| 24 | 4 | Window log, 10..17 |
| 28 | 4 | Hash log, 6..12 |
| 32 | 4 | Chain log, 6..12 |
| 36 | 4 | Search log, 1..8 |
| 40 | 4 | Minimum match, 3..7; effective key width is clamped to 4..6 |
| 44 | 4 | Target length, exactly 0 |
| 48 | 4 | Strategy, exactly 6 |
| 52 | 4 | Initial logical frontier, exactly 0 |
| 56 | 4 | Initial logical window floor, exactly 0 |
| 60 | 4 | Initial logical prefix floor, exactly 0 |
| 64 | 16 | Four reserved zero words |

Then store K fixed 8-byte operation records (kind u32, logical query u32), followed
by exactly N source bytes and EOF. The maximum packet is 133200 bytes. Names and
expected answers do not enter this binary protocol. Only kind 1 (`find`) is
initially admitted. Queries are strictly increasing, each in 0..N-8, with the
physical eight-byte hash footprint ending no later than N. An empty source,
unknown kind, repeated/backward query, nonzero reserved field, extra byte or
truncation fails before constructing C state. No caller-supplied hash cells,
links, offset seed, floor changes or arbitrary frontier are accepted.

A query calls the original `ZSTD_BtFindBestMatch` with no dictionary and effective
key width, including its early-return and update path. Every call initializes
the offset-base variable to 999999999, matching all three search call sites in
`ZSTD_compressBlock_lazy_generic`. A skipped query returns length zero and leaves
that seed unchanged. Exposing a caller offset seed would introduce a different
cost-selection contract and is deliberately excluded from version 1.

The helper owns one source arena, one head allocation and one node allocation,
plus independent before-image arrays and a bounded output buffer. The source
arena has a 64-byte leading region, N logical bytes, and a 2048-byte initialized
suffix, with 64-byte exterior guards. `base = source - 2` lies within the leading
region; initial `dictLimit`, `lowLimit` and `nextToUpdate` are two. `nextSrc` is
logical source end. `dictBase` points into that same arena although the selected
branch never reads an external dictionary. Source and all guard/padding bytes
are checked unchanged after every operation.

Each head array has 2^H initialized u32 cells. Each node array has 2^C initialized
u32 cells, interpreted as 2^(C-1) pairs. Before-images have exactly the same
lengths, never alias current maps, and start initialized. Every size addition,
shift, multiplication and alignment increment is checked before allocation.
Use naturally aligned or explicitly 64-byte-aligned storage with retained exact
allocation bases, 64-byte guards, and an aggregate 4-MiB live cap including
output/before-images. Partial failure releases every already-created owner once.
No success trailer may be emitted after allocation, output or guard failure.

## Transition semantics and observations

Do not reimplement the tree algorithm in C instrumentation. The helper may
snapshot state before/after each call, inspect maps, compare guards, and copy the
reported candidate; it must call the unchanged pinned implementation for search.
The first version has no internal branch counter inserted into upstream source.
Coverage names describe intended input families until actual observable
transitions establish a witness. An end-map alone does not establish the exact
chronological branch path or comparison count.

Emit exactly one metadata record, K operation records for padding pass zero
(0x6a), K operation records for pass one (0xc3), then one completion record.
Each pass constructs fresh zero-initialized tables and the same initial frontier;
pass one never inherits pass zero state. There are exactly 2K+2 newline-terminated
records and at most 128 MiB stdout. A streaming per-record output buffer is
bounded to 256 KiB and included in the live-allocation cap; the helper does not
retain the entire output. All scalar types are exact integers,
booleans are not integer substitutes, keys are unique and finite, and stdout
must end in a complete newline with no trailing non-record data.

Metadata binds protocol version, upstream revision, scalar parameters, source
length and FNV-1a-64 hash (standard offset basis and prime), operation list, physical bias, initial offset-base seed, table
sizes, arena sizes and pointer-independent owner counts. Each operation binds
ordinal, query, padding pass, before/after frontier, effective moving window
floor, tree floor, selected length and raw offset base. Record complete before/
after head and paired-node maps, not just a hash or a sparse set of changed
cells. Raw cells retain C's zero/one/biased-position meanings for independent
validation; source positions are reported separately as logical indices.

The reader checks complete state continuity, exact array lengths and every cell
range. The unsorted marker one is accepted only in the second node cell; head
cells and first-node cells may only be zero or admissible biased source indices.
Second cells that are not markers can encode a reversed pending chain as well
as sorted children. Range validation must not pretend those are always sorted
edges. A head/link can become stale relative to the current window or ring; it
must not be rejected merely for being stale if the reference stores it without
following it. Conversely, any selected candidate must be strictly before its
query, above the active C window floor, within the source extent and byte-equal
for the full reported length. A zero-length result retains the initial offset
seed. A nonzero raw offset base decodes through the standard offset bias and
must identify exactly the copied source match.

No reader generates expected best-match decisions by running the new Rust tree
or by trusting names. It verifies packet identity, arithmetic, map continuity,
source replay and output/state bounds; complete C maps remain observational
expectations for Rust. Captures of the same packet must be equal across both
padding fills and normal/instrumented C builds. This structural audit does not
prove that every semantically plausible altered cell is impossible; exact
artifact hashes and independently frozen original captures provide that binding.

## Bounded work and safety review gates

The query wrapper checks `ip < base + nextToUpdate` before any update. Otherwise,
updates only insert source positions preceding that query. At the inclusive
last query, hashing ends exactly at N; match counting ends at logical N and any
lexical discriminator is read only when a match stops before N. The source cap
keeps four-times length gains within signed 32-bit arithmetic. The helper must
review the pinned upstream's high-bit preconditions with the fixed nonzero seed,
not assume the parameter validator establishes them.

The node floor is `max(0, biased_query - node_mask)`; the unsorted-list limit is
the maximum of that floor and the moving window floor. The initial C floor is
two, so logical source position zero is a valid stored value but is excluded by
a strict `candidate > windowLow` search at that initial floor. This distinction
must survive translation to EMPTY sentinels and zero-based Rust positions.
The tree may compare a candidate outside the current node ring but must stop
before following its overwritten pair. The sampled farthest match end can move
`nextToUpdate` ahead of later queries; those later calls remain visible as
unchanged-map skipped operations.

Normal and ASan/UBSan helpers are separately built from the same pinned clean
source. All linked codec translation units and the helper are instrumented in
the latter; compilation and linking use `-fno-sanitize-recover=all`. Disclose
existing upstream no-sanitize attributes. Use timeouts, output-size limits,
whitelisted environment, exact compiler/executable/source identities and a
recorded command manifest. Preflight positive syntax and malformed packet/audit
controls before capturing observations. Forged maps are consumed only by offline
readers, never passed into C tree routines whose preconditions use assertions.

A later broader protocol or ordinary-frame protocol must be versioned if it
adds prefix/window schedules, arbitrary operation types, full block parsing or
new caps. Preserve these first capture bytes and their original validator. Public
admission is a separate successor gate after state and frame integration.
