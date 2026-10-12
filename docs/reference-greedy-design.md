# Bounded no-dictionary reference Greedy

This is the next reference-strategy module after the bounded Fast/DoubleFast
milestones. The analysis below reads facebook/zstd v1.5.7 at
f8745da6ff1ad1e7bab384bd1f9d742439278e99. It does not claim new C captures,
Greedy execution, performance or complete upstream migration. Module 13 validation and measurement are complete; Module 14 implementation
is still pending.

## Scope and admission

Add resolved `ReferenceStrategy::Greedy` to the existing known-size one-shot
request APIs. Keep named Greedy/Chain/Lazy policies and all existing frame and
allocation baselines unchanged. Numeric strategy identity does not mean the
existing independent named parser is a reference implementation.

Automatic reference search selection is based on resolved window log: row search
when windowLog > 14, hash-chain search otherwise (`zstd_compress.c:225-250`).
It does not depend on SIMD availability. The public request currently has no
row-mode override; both branches must exist before enabling Greedy publicly.
Choose after actual known-source adjustment; source hints remain subordinate.
No dictionaries, reusable numeric contexts, externally supplied sequences,
workers, LDM, stable output buffers, block splitting or Lazy/Lazy2 lookahead are
introduced by this module.

Proposed operational caps are windowLog <= 21 and hashLog <= 19. These cover the
pinned large-source level 5 Greedy preset (21/18/19/3/5/2). Chain search also
requires chainLog <= 18 and searchLog <= 8 (at most 256 chain visits per probe).
Row search allocates no chain and caps attempts to 2^min(searchLog,rowLog), at
most 64; its unused chainLog and searchLog above 6 need no artificial allocation
or search-budget rejection. This asymmetry must be documented and tested after
resolution. Minimum match is clamped 4..6 for either branch; target length does
not change this parser or select Fast-only raw literals. Reject unsupported
strategies before selected-strategy resources, and validate every layout and
complete output bound before cloning or allocation. Preserve existing Fast and
DoubleFast caps and precedence independently. A new private layout supports
hash log 19 without widening SearchKeyLayout's existing named/Fast/DoubleFast
domains. C's unused minMatch = 3 hashTable3 owner need not be copied into
Rust; no C/Rust allocation-count equivalence is claimed.

Keep checked usize positions, the known-source u64 domain and fallible output
reservation. Oracle inputs will be bounded to a few MiB; no C near-4-GiB index
reduction equivalence is implied. No silent source truncation or arbitrary new
whole-input cap is permitted.

## Ownership and representations

| Rust role | Ownership and invariants | Pinned upstream counterpart |
| --- | --- | --- |
| Greedy execution plan | Copy value: resolved parameters, backend, effective width, row/search dimensions, block limit and checked byte totals; no owners | Applied cParams and ZSTD_resolveRowMatchFinderMode |
| Borrowed Greedy block | Complete immutable source plus checked range, retained prefix floor and window; every byte operation checks actual extent | ZSTD_window_t, ZSTD_compressBlock_lazy_generic depth 0 |
| Reference chain index | Independently owned Vec<usize,A> hash heads and cyclic predecessor cells; initialized invalid sentinel, exact owner per Vec, source positions only | hashTable, chainTable and nextToUpdate in ZSTD_HcFindBestMatch |
| Row match table | Independently owned initialized Vec<usize,A> positions and Vec<u8,A> tags/heads; 2^hashLog cells each; each row has 16/32/64 cells with position 0 reserved for the head tag | hashTable, tagTable and ZSTD_row_nextIndex |
| Row layout/hash | Copy scalars: rowLog = clamp(searchLog, 4, 6), rowHashLog=hashLog-rowLog, tag width 8, nonzero full hash width <= 32 and salt | rowHashLog, ZSTD_hashPtrSalted |
| Match candidate | Copy source/current/sample positions, length and explicit/repeated intent; no source pointer | offBase, matchLength and start |
| Working search state | Own exactly the chosen backend, candidate literals/sequences and logical next-to-update/skipping state; source borrowed, committed entropy borrowed only for each parse | matchState/seqStore versus prevCBlock/nextCBlock |
| Shared frame/entropy owner | Reuse existing transactional frame publication, source flags and independent entropy owner; no inactive backend | Existing reference frame loop |

The current named MatchChain stores generation-tagged insertion-order nodes and
has a 64-visit policy. It cannot substitute for the reference cyclic predecessor
semantics. Use a separate reference-chain leaf with `unsafe-reference-chain`.
Give the two-owner row table its own `unsafe-row-table` backend. Both default
implementations use only checked slices/indices; optional unsafe accessors keep
identical validation, error, initialization and ownership behavior. Check every
index and all fields of a write before its first mutation. Each unsafe function
needs complete validity, bounds, initialization, aliasing, alignment and lifetime
contracts; each call site must prove those conditions. Do not add unsafe work
to plain value records solely to create a second representation.

For row tables, requested payload bytes are
(usize::BITS/8 + 1) * 2^hashLog, not the C u32 table footprint. Chain tables need
size_of::<usize>() * (2^hashLog + 2^chainLog). Layout, sum and owner construction
are checked separately. Independent allocators may allocate from different
domains; failed second-owner construction releases the first through its exact
owner. Moves, swaps and resets retain this invariant. no_std+alloc is required.

Avoid translating C prefetch/SIMD plumbing into pointer-rich Rust. A scalar tag
scan in cyclic candidate order can implement the same row contract safely.
An uncached hash calculation is semantically sufficient if all sampled positions,
insertion order and the C search-tail boundary remain identical; evaluate this
choice with frozen direct observations before implementation. An optional later
cache must be a separate justified optimization, not inferred from the C layout.

## Distinct search invariants

The reference chain inserts positions from nextToUpdate up to, excluding, the
query; lazy skipping inserts only the first pending position. It then sets
nextToUpdate to the query and searches the current hash head. Each predecessor
occupies a cyclic slot based on the source coordinate. C starts valid indices
at 2. Define the bias/modulo translation explicitly when comparing full link maps;
do not compare an unshifted Rust slot map to the biased C map. Relative indices
must never be widened from an already truncated u32.

A chain probe spends at most 2^searchLog visits. It accepts candidates at the
inclusive moving floor max(retained_low, current-window), starts best length 3,
uses strict length improvement (first candidate wins ties), and stops following
predecessors at current-chainSize. Check past/future/cyclic/stale values before
source reads, independent of trusted input bytes. The existing named chain's
visited-node semantics and tags do not establish these invariants.

Row insertion cycles backward through 1..rowMask, reserving cell 0 for the head.
Candidates are selected from matching 8-bit tags in head-relative cyclic order,
skipping cell 0; an expired oldest candidate terminates collection. Bound the
snapshot to at most 64 positions and collect before inserting the current probe.
The new current tag/position is then stored and nextToUpdate advances by one;
only afterward compare the saved candidates for strict longest match.
In lazy-skipping mode, row search inserts only the current probe, while chain
search catches up by inserting its first pending position. Check
table/candidate/source indices even when tags collide or initialized
sentinels match the query tag.

The row hash is salted before taking high bits: four-byte hashing uses the low
32 salt bits; widths 5/6 use 64-bit arithmetic and explicit little-endian loads.
A fresh C compression context advances its zero salt/entropy once through
ZSTD_bitmix (zstd_compress.c:1970-1982,2032-2051); it is not generally salt 0.
Freeze the observed fresh-context salt before choosing a Rust initial value.
Context reuse and dictionary salt 0 are separate future contracts. Record C
hashSaltEntropy updates for trace attribution without exposing an unused public
reset capability.

On a gap strictly greater than 384, cached row update inserts the first 96 and
last 32 pending positions; exactly 384 does not skip. The ordinary build-sequence
wrapper also advances nextToUpdate after a cross-block gap > 384, preserving at
most 192 positions (zstd_compress.c:3292-3297). Freeze these distinct transitions,
including 383/384/385 and 575/576/577 gaps. Do not collapse them into a single skip
rule. Compression-probe failure enables lazy skipping only when
((current-anchor)>>8)+1 >8; matches disable it again.

## Greedy parsing and publication

The common lazy parser at depth 0 probes repeat1 at current+1 first and accepts
it immediately if at least 4 bytes match. Otherwise search at current and accept
only length >= 4. On a miss, advance by ((current-anchor)>>8)+1 and update skipping.
Explicit matches extend backward to the anchor/prefix bound and rotate repeats;
repeat matches retain their distinct wire intent. After a sequence, consume
immediate repeat2 matches while the inclusive cleanup bound permits them,
swapping repeat1/repeat2 for each one. Restore initially invalid saved repeats
using the same independent raw-parser versus wire-repeat treatment as existing
reference parsers.

Chain search ends its main loop at end - 8; row search uses end - 16 because the C
hash cache has 8 entries. Preserve strict main-loop and inclusive repeat cleanup
comparisons even if the Rust implementation omits prefetch/cache. For tiny
blocks, represent an unavailable search limit explicitly rather than subtracting
before the source allocation. Test exact slices without spare readable padding.

Greedy's search floor is evaluated at each probe, while the retained prefix
floor is advanced by the ordinary wrapper at block start. DoubleFast's ordinary
candidate floor is derived from block end. Do not reuse DoubleFastBlock's
ordinary floor for Greedy. Entry repeat validation uses the current position's
prefix floor; all subsequent distances and extents remain checked.

The <7-byte ordinary bypass, tables retained across raw/RLE blocks, candidate
entropy transaction and committed-repeat publication must be tested separately
from direct parser calls. Parse all relevant blocks before deciding emission;
only successful compressed-block append commits entropy/repeats. Allocation or
append failure poisons the workspace after releasing any pending candidate.
The existing shared plain frame metadata already admits window log 21;
no wider window is needed for this module. Preserve its header and output-bound
behavior and the narrower Fast admission. The gain rule (size>>6)+2 remains valid for Greedy; independently
verify policy selection and owner order.

## Independent evidence before implementation

Freeze a new versioned direct protocol before C execution: source prefix and
block extents, explicit branch, scalar parameters, initial full heads/links or
rows/tags, salt, pending-update position and raw repeats. Return full sequences,
literals, raw and logical repeats, update/skipping state, both owner maps and
bounded trace counts. Define every arena bound, zero-size owner, initialized
padding, nonoverlap, short-input limit and output capacity in the protocol.
The C row helper must provide 64-byte-aligned hash storage and tag rows
aligned to their row width, as required by ZSTD_row_prefetch. Account for
alignment padding separately from source padding and allocation guards;
ordinary malloc alignment alone does not prove those requirements. Scalar Rust
row access needs only the natural alignment of its owned element types.
Review actual C helper allocations against that proof before executing it.
Build every used C translation unit under ASan/UBSan; record exact source/header,
compiler, flags, object/link identities and normal/instrumented observations.

Include width 3/4/5/6/7 dispatch, window 14/15 selection, row widths 16/32/64,
chain and row hash collisions, equal-length tie winners, head wrap, slot reuse,
zero-tag/sentinel cases, inclusive/exclusive floors, expired/future candidates,
repeat overlap, long gaps, skipping transitions, partial source windows and
nonzero salts. Capture ordinary trajectories and complete frames independently
of the direct parser, including raw/RLE changed-repeat discard. Do not use
current Rust output to manufacture any C expectation. Reject malformed or
changed capture data offline with subprocess creation disabled.

Public execution follows private parser/state/complete-frame checks. Before
admission changes, freeze successor v3 mappings over original selectors, both
the 92-case Fast-derived interop inputs and 159-case broader DoubleFast inputs,
and original numeric loops, without rewriting v1/v2 artifacts. The old broader
adapter's 2-MiB frame-window cap is sufficient for this scope. Its frozen recipe
statuses must remain unchanged; a separately versioned successor policy and
Greedy cases need review before reusing its bounded transport. Actual newly
admitted Greedy inputs need additive positive C execution. Existing blocked dictionary hash 29/chain 25,
source-hint/context-order, in-place, caller-capacity, stable-workspace and external
corpus obligations remain visible; no source-file status promotion follows from
this one no-dictionary parser.

Complete every-feature/all-feature, no_std, allocator ordinal, malformed input,
strict Clippy and focused interpreted/no-atomics checks. Freeze deterministic
row/chain workloads and unchanged Fast/DoubleFast baseline comparisons before
measurement; run clocks only after task-owned CPU work has finished. Report
byte/owner equality and every sample without assuming unsafe speedups.

## Source-only successor forecast

A separate planning calculation over the unchanged v2 TSVs and pinned preset
rows predicts 62/15/4 selector success/unsupported/advanced records, adding only
unsupported-57. The 92-case historical interop set would become 86/3/3, adding
unsupported-strategy-3. Empty original loops would admit 6 of 24 profiles and the
30-byte initial-match loop 4 of 19, adding level 4 in each. These are source-derived
expectations under the proposed caps, not new C applied-parameter observations
or successful Greedy executions. Capture and freeze a reviewed v3 fixture only
at the corresponding work item. New row-based cases are needed independently:
these small newly admitted historical cases exercise the chain branch.

The [source-plan record](evidence/2026-10-12-reference-greedy-plan.json) pins
the reviewed source and includes the planning calculation and its inputs.
It is not an oracle capture or a replacement for the later v3 admission fixture.
