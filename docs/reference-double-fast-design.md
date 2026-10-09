# Bounded no-dictionary reference DoubleFast

Module 13 plan, prepared 2026-10-09 by read-only source inspection of the Module12 public API,
which remains unchanged at root revision `9ef133b` during this review. No DoubleFast implementation,
fixture capture, compilation, benchmark, or test run is claimed by this review.
Implementation and C capture start after Module 12 final validation and measurements.
The working reference is facebook/zstd v1.5.7, revision
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`, outside the repository at
`/tmp/zstdoxize-upstream-1.5.7`. Upstream paths/lines below are relative to that
checkout. Source identities:

- `lib/compress/zstd_double_fast.c`: SHA256
  `9f6d003c2fe1208737bec685910324220424ba47d0323fa30d9d5f53c50e91da`.
- `lib/compress/clevels.h`: SHA256
  `d764daa89b7a636d26fe96ff3536c5075e9867fb5dddbf8c9d57023a8ff4a155`.
- `lib/compress/zstd_compress.c`: SHA256
  `10c315ae609d49b2fc431521aa95908086e69fd917991c5ed9a875af3d1208e8`.

## Scope and admission recommendation

Add the actual no-dictionary DoubleFast algorithm to the existing opt-in
`compress_at_level[_in]` and `compress_with_parameters[_in]` APIs. Keep `compress`,
`Encoder`, the named Greedy/Chain/Lazy policies and MatchSearchOptions unchanged.
Do not implement DoubleFast by combining two independent Greedy passes or by
selecting the existing Chain parser. Existing source-size and scalar request
resolution stays authoritative; dispatch only after it has produced an effective
`ResolvedCompressionParameters` record.

Recommend these initial effective DoubleFast limits:

| Resource | Proposed admission | Meaning |
| --- | --- | --- |
| Window log | 10..=21 | At most 2 MiB eligible contiguous borrowed history; needed for unchanged default-level profiles on sufficiently large inputs. |
| Long hash log | 6..=18 | `2^hashLog` initialized usize positions, eight-byte keys. |
| Short hash log | 6..=18 | `2^chainLog` initialized usize positions; this is a second hash table, not a link chain. |
| Minimum-match field | 3..=7 | Dispatch 3 through 4; short key width is max(4,minMatch), long width is always 8. Emitted matches may be four bytes. |
| Search log / target length | Existing validated scalar domain | No noDict DoubleFast effort/acceleration/storage control: retain the requested/resolved values but do not invent consumers. |
| Effective block limit | min(selected window,128 KiB) | The source remains one immutable borrowed slice. |

Cap each table with its checked layout, and check their sum before allocation.
The requested table payload is
`size_of::<usize>() * (2^hashLog + 2^chainLog)`: at the proposed maxima this is
4 MiB on 64-bit and 2 MiB on 32-bit targets. This is not a whole-context memory
estimate: literals, sequences, entropy scratch, frame output, allocator
bookkeeping and excess returned capacity remain separately owned/reported.
Check `Layout::array::<usize>` for each table, checked sum/shift/conversions and
the complete raw-frame output bound before cloning/allocating. Keep the existing
Fast window-log-20 policy unchanged; do not accidentally widen Fast admission
while adding DoubleFast's distinct window-log-21 limit.

Keep the existing checked usize slice-index/source-size domain and output-bound
rules; do not introduce an undocumented whole-input cap or u32 truncation. The
parser's active reference distances are bounded by the admitted window. The
oracle campaigns should initially bound complete sources to a few MiB and
explicitly exclude claims about reproducing C's near-4-GiB index-reduction
trajectory. No additional public source-length cap is introduced. The checked slice-index
domain and fallible complete-output reservation remain distinct from the
2-MiB history window and actual table allocation caps.

Error precedence remains: source-domain validation and scalar resolution,
unsupported algorithm, then supported-algorithm resource/layout/output bounds,
then allocation. Fast and DoubleFast are supported only once their execution
paths exist; other reference strategies remain UnsupportedFeature. A large
valid DoubleFast profile outside these caps becomes ResourceLimit, never a
clamped profile. Tiny known inputs may resolve large requested dimensions down
to admissible effective dimensions. Empty input still gets validated/resolved,
but after the output allocation must not allocate either match table.

### Four tiers and levels 0, 2, 3

`zstd_compress.c:7759-7788` chooses the four preset rows and maps level 0 to
level 3. The table below lists the unadjusted preset triples; normal known-size
adjustment can reduce their window/hash/chain dimensions afterward. Tuple order
is W/C/H/S/minMatch/target/strategy.

| Source-size tier | Level 2 | Level 0 and level 3 |
| --- | --- | --- |
| <=16 KiB | 14/14/15/1/4/0/Fast | 14/14/15/2/4/0/DoubleFast |
| >16 KiB, <=128 KiB | 17/13/15/1/5/0/Fast | 17/15/16/2/5/0/DoubleFast |
| >128 KiB, <=256 KiB | 18/14/14/1/5/0/DoubleFast | 18/16/16/1/4/0/DoubleFast |
| >256 KiB | 20/15/16/1/6/0/Fast | 21/16/17/1/5/0/DoubleFast |

Sources: `clevels.h:28-32,54-58,80-84,106-110`.
With the recommended caps, unmodified levels 0/1/2/3 and negative Fast levels
are executable across all four tiers, subject to ordinary allocation/output
limits. A single 262145-byte example is insufficient to prove the window-21
case: known-size adjustment reduces that particular window. Include >1-MiB
and >2-MiB boundary sources in native coverage.

Do not use a numeric allowlist. Level 4 is DoubleFast only for
16 KiB < size <=128 KiB and size >256 KiB; it remains reference Greedy in the
other tiers. Admit its resolved DoubleFast cases within caps, reject its other
strategies, and preserve explicit strategy-override precedence. No assertion
that numeric compression ratios monotonically improve is justified by the
independent Rust entropy implementation.

## Exact parser decisions, and the important differences from Fast

The new parser corresponds to
`zstd_double_fast.c:105-324` (`ZSTD_compressBlock_doubleFast_noDict_generic`),
with exported dispatch at `:568-585`. Dictionary parsers at `:328-547` and
`:610-775`, and dictionary table filling at `:17-99`, are outside this module.

1. **Two independent collision domains.** `hashTable` is the long table sized
   by hashLog and hashed with width 8; `chainTable` is the short table sized by
   chainLog and hashed with the dispatched width. Both store only one sampled
   position per bucket. C allocates the latter for DoubleFast
   (`zstd_compress.c:255-263`); no circular nodes, predecessor links, visit
   budget, Chain dictionary seed or `unsafe-match-chain` data path is involved.
2. **Different scheduling.** Each outer search starts `step=1`, `nextStep=ip+256`
   and `ip1=ip+step` (`:176-184`); `kSearchStrength=8` is in
   `zstd_compress_internal.h:32`. After the threshold, the step increases by
   one and the threshold by 256 (`:236-249`). Target length does not affect this
   noDict parser. Do not copy Fast's initial target-derived step, paired-probe
   schedule or 128-byte threshold. The comment describing a later long match
   at "+1" must not override the actual `ip1`, whose gap can grow above one.
3. **Cached read/write ordering.** Read the current long candidate before the
   loop; read the short candidate before storing the current position into both
   tables. Check repeat1 at `ip+1` before the ordinary long/short paths. Compute
   the next long key, check current long candidate, then read the next long
   table entry. That next read can observe the current long write when buckets
   collide. Lookup/replace is not interchangeable here (`:186-235`).
4. **Match preference.** Current long match verifies eight bytes; current short
   verifies four despite its wider hash. Only after a short match is found is
   the later long candidate counted. Prefer it only for strictly greater match
   length; ties preserve the short match, without subtracting the extra literal
   gap (`:266-288`). Current-long and current-short candidates accept equality with
   the prefix floor: `ZSTD_selectAddr` at `zstd_compress_internal.h:627-644`
   selects the candidate for `index >= lowLimit`, including its x86 branch.
   Only the later-long candidate has the explicit strict `idxl1 > floor`
   condition at `zstd_double_fast.c:275`. The nearby C comment must not
   override the helper's actual comparison. Backward extension may finish exactly at that floor:
   `:222` and `:286` check `match > prefixLowest` before decrementing. Its other
   bound is the literal anchor, which the source position may also reach.
   Preserve the original sampled `curr` for post-match
   insertion even if the selected start moves forward/backward.
5. **Repeat intent.** The `ip+1` repeat writes offBase 1; explicit matches write
   distance+3. After a match, immediate zero-literal matches use the second
   local repeat, swap the first two cells, and again write offBase 1
   (`:199-205,:290-320`). Use existing `SequenceWithOffset`/`OffsetIntent`; do not
   let a canonical cheapest-distance planner choose a different repeat intent.
6. **Post-match table contents.** Conditional long-table insertion at `ip1` is
   performed only when `step<4`. If there remains an eight-byte search tail,
   write long(`curr+2`), long(`ip-2`), short(`curr+2`), then short(`ip-1`),
   preserving collision-sensitive order within each table. Immediate repeat-loop starts enter both tables.
   Full collision-sensitive before/after tables matter (`:294-321`).
7. **Bounds have two roles.** Current ordinary candidates accept the block-end-derived
   prefix lowest index; the later-long probe requires an index strictly above
   that floor, as specified above. Initial repeat disabling uses the
   current-position-derived available prefix length (`:118-125,:165-173`).
   Preserve saved-repeat restoration and rotation at cleanup (`:252-263`).
   Replaying a repeat requires positive distance <=window and a source position
   at/above the retained prefix floor; do not incorrectly impose the stronger
   ordinary block-end floor on every repeat. Index zero is a normalized valid
   source position only when the algorithm permits it; an empty C table slot
   remains invalid, not source byte zero.
8. **No raw-literal acceleration rule.** Automatic literal suppression checks
   `strategy==Fast && targetLength>0`, not DoubleFast
   (`zstd_compress_internal.h:685-698`). A forced DoubleFast request with a
   positive target must still use normal literal selection. SearchLog and
   targetLength should have positive invariance tests for the same effective
   DoubleFast profile. Both strategies share minimum gain `(size>>6)+2`
   (`:673-683`) but not all plan fields.

Use Option/index eligibility instead of C dummy-pointer selection; a hash hit
never proves that source bytes are accessible. Checked additions and subtraction
preconditions must cover every current/next position, match extension and key.
Physical C speculative reads/prefetch pointer expressions are oracle setup
requirements, not a reason to overread the safe Rust source.

## Ownership and proposed private representation

| Record/owner | Minimal proposed role and invariants |
| --- | --- |
| `DoubleFastExecutionPlan` | Copy record of resolved parameters, two layouts, each/summed checked payload sizes, source/window/block/frame metadata. No owner and no unsafe alternative. |
| `DoubleFastBlock<'a>` | Borrowed complete immutable input plus block range, retained floor, window and checked layouts. No stored C pointer, source copy, dictionary attachment or self-reference. |
| Parser cursor/result records | Checked scalar positions, current/next cached buckets and candidates, step/threshold, two saved repeat cells, final literal count, separate raw-parser and semantic wire triples. No allocation/unsafe variant. |
| `DoubleFastMatchState<'a,A>` | Complete `MatchTable<A>` long owner and complete `MatchTable<A>` short owner; `Vec<u8,A>` literals, `Vec<SequenceWithOffset,A>` sequences; borrowed source, position, prefix floor, failed flag and plan. Each child retains its exact allocating value. |
| Existing `BlockEntropyEncoder<A>` | Reused literal/FSE/bitstream/payload owners and candidate commit/discard. No source borrow or duplicate dictionary/history owner. |
| Reference dispatch record/state | A small static enum over Fast/DoubleFast plans and states, or equivalently explicit matched branches; never dynamic dispatch, allocator erasure, C binding or an unused allocation for the inactive algorithm. |

`MatchTable` already has checked `lookup` and `store`, sufficient to preserve
DoubleFast's read/write order. Its equivalent `unsafe-match-table` accessors
should service **both** table instances. Their owned layout identity and live
initialized length checks remain mandatory. No new unsafe feature is warranted
for a pair of existing owners, plain cursor records, or a safe parser. A future
packed/tagged dual table or genuinely new pointer representation would need its
own design and independent feature; do not add that representation now.

One narrow layout extension is required: `SearchKeyLayout::for_fast` currently
admits widths 4..7 only, while `new` admits width8 but logs>=10. Add an explicitly
private checked long-table constructor for width8/log6..18 (or a clearly named
common reference-hash constructor with each parser retaining its own width
validation). Preserve the current Fast/named bounds. Existing C hash vectors
cover width8 for logs10..18; new direct captures must also independently cover
width8/log6..9 rather than assuming those saved vectors already exist.

Build the two fresh tables with separate fresh allocator clones; borrowed
`&NonCloneAllocator` remains usable. Never copy cells into a vector with another
owner or transfer an allocation between clones. Construct/from-parts validation
must check long width8, short width4..7, both exact bucket counts and the admitted
plan before parsing. Before the first table mutation, validate source range,
repeat snapshot and owner/layout identity, then reserve both output vectors.
If the second reservation fails, the first capacity may have grown but logical
output/table contents remain unchanged. Subsequent parser failures poison the
persistent workspace rather than promise a new whole-parser rollback contract.
Check all observed allocation ordinals, including failure of the second table
construction after the first succeeded, with exact final release.

## Persistent state and entropy publication

Retain both sampled tables across consecutive blocks of the borrowed source;
never densely reseed all preceding source bytes or call either table's reset
between blocks. Reset/construct once per independent frame. Update the retained
floor from block start and selected window before the parser applies its own
block-end ordinary-match bound. Tiny blocks of 1..6 bytes bypass fresh parsing
and keep stale scratch inaccessible; an empty continuation changes nothing.
Sources: `zstd_compress.c:3264-3308,:3415-3426`.

Seven- and eight-byte blocks still enter the parser, replace literal scratch,
perform saved-repeat cleanup and return all literals, but avoid hash access.
A nine-byte block can search when a prior prefix keeps `ip == block_start`;
a fresh block whose start equals the prefix floor skips one byte and needs ten
bytes to enter the loop. Pair lengths 7/8/9/10 with and without prior history.
Only lengths 1..6 take the ordinary-frame stale-scratch bypass. Do not reuse
Fast's paired-search termination guard for DoubleFast.

Three state boundaries must remain distinct:

- Parsing mutates sampled search tables and produces candidate literals,
  sequences, raw first-two repeat cells and a separately replayed full wire
  triple. Hash mutations survive raw/RLE fallback.
- Ordinary raw/RLE emission does not publish candidate repeat or entropy state.
  Compressed emission publishes only after complete output reservation and
  append. C's first block is not converted to block RLE; later uniform blocks
  use the <25-byte/effective-zero compressed-size gate after entropy work
  (`zstd_compress.c:4383-4449`). Reuse the existing Rust emission policy while
  preserving its independent entropy qualification.
- C's direct parser updates only rep[0]/rep[1], leaving the raw third cell stale.
  Rust must retain a complete semantic wire triple. Compare the first two raw
  cells and the full independently replayed wire triple in ordinary trajectory
  fixtures, just as the existing Fast state tests do; never silently claim all
  three raw C cells equal the wire decoder state.

The collector is a separate trajectory: `zstd_compress.c:4408-4411` confirms
candidate state when collecting even though it returns no compressed block.
`ZSTD_copyBlockSequences` at :3429-3505 has the already documented raw-16-bit
literal-length repeat-update nuance for long literals. Capture disagreements
as observations/limitations; do not repair its output or use it as the sole
wire-repeat oracle. Ordinary C and independent Rust entropy may choose different
raw/compressed blocks, so identical persistent parser histories are asserted
only when replaying the same specified publication schedule/incoming repeats,
not across every independently emitted Rust/C whole frame.

## Narrow integration path

Avoid copying the entire Fast framing/entropy implementation. Before enabling
DoubleFast publicly, extract a shared plain frame-settings record (known size,
window log/size, block limit, reference frame flags) from the current
Fast-specific header boundary, then a small explicit search dispatch. Resolve a
request once and pass its checked plan into the selected workspace. The common
entropy entry point should accept the already checked raw-literal policy and
shared gain rule; currently it is named `prepare_fast` and imports
`FastExecutionPlan` only for policy. Rename/extract that narrow policy boundary
in a behavior-preserving checkpoint, with frozen Fast/named output and allocator
count comparisons. Do not derive a DoubleFast plan by lying that its strategy is
Fast or by converting it into MatchSearchOptions.

Preserve original-owner output, exact child allocator values, preallocation
error ordering, no source/request borrow in returned vectors, poison rules and
empty-frame scratch avoidance. The existing public API signatures suffice;
there is no new live level setter or numeric streaming Encoder in this module.

## Independent capture requirements before Rust parser implementation

Freeze independent C observations before new parser code can supply expected
values. Use unchanged exported `ZSTD_compressBlock_doubleFast` from the pinned
source, reviewed bounded initialized arenas and guarded sequence/literal output.
Reuse the *design* of Fast's direct packet/importer, not its result records.
Record both table layouts, normalized entries, complete before/after hashes and
canonical deltas; reconstruct every cell independently. Zero C slot remains an
invalid normalized cell; nonzero source indices use the explicit +2 mapping.
Every accepted sampled key must fit the initialized selected-byte domain.

The setup review must newly account for eight-byte long reads, the four-byte
short comparison, the actual short-hash physical read, the dummy[10] object,
`ip1`/nextStep expressions, architecture-specific `ip+256` prefetch, wildcopy
slack and all guard placements. Strictly validate packet sizes/EOF, source and
prefix lengths, both table capacities, seed order and ranges, repeat/window
bounds, long-length markers and output record cardinality. Initial seed keys
should fit entirely before the block start for the first ordinary-like cases;
explicit constructed variants must identify their distinct scope. Compare two
initialized padding patterns, independently replay source/literals/matches and
full wire repeats, and prove the final table maps rather than infer chronology
from a final hash.

Directed families should establish observed witnesses for:

- minMatch3/4 equality; short widths4/5/6/7 with long width8; long/short logs6..9
  and conventional independent/asymmetric dimensions; a few max-table cases;
- exact 7/8/9/10-byte cleanup/search pairs with and without prior prefix,
  verifying both unchanged tables and fresh literal/repeat cleanup where no
  hash probe occurs;
- exact-floor current-long and current-short acceptance, later-long rejection
  at equality, and floor-reaching backward extension;
- collisions in only one table, both independently, and current/next long slots;
- repeat-at-ip+1 precedence, current long match, short match retained on equal
  length, later long match winning strictly, and backward extension including
  an observed match that starts strictly above the prefix floor and extends
  exactly to it (this witness deliberately starts above the floor so backward extension is exercised);
- step thresholds255/256/257 relative to the actual search start, growing gaps
  and an isolated step3-versus-step4 pair demonstrating the conditional
  next-long store gate; preserve the observed store behavior separately from
  source-derived scheduling reasoning;
- each complementary insertion and immediate zero-literal repeat loop;
- initially unavailable repeats, restoration/rotation, duplicate repeat offsets,
  active floor boundaries and fully expanded long literal/match lengths;
- target/search-field invariance without erasing their recorded resolved values.

Keep direct constructed state, ordinary-frame publication and public collector
as three separate captures. Ordinary-frame fixtures need raw/compressed/RLE
transitions, first-block uniform data, tiny/empty pieces, both retained tables,
window eviction and nontrivial discarded repeat candidates. Public whole-frame
interop should separately record requested/query/applied parameters and exact
C decoding; it must not call internal-begin observations public resolution.

## Small independently verifiable work items

1. Record scope, ownership, admission/resource choices and exact source map in
   project design/TODO before code. Freeze current Fast and named-policy bytes
   and relevant allocator counts for later regression comparison.
2. Add the bounded direct C helper and tiny hand-readable records, with external
   independent setup review and normal/scoped sanitizer capture. Freeze before
   Rust parser implementation. Keep observed and unobserved branch claims apart.
3. Expand directed direct families and strict offline validator/negative controls
   as a separate fixture checkpoint, including two-table reconstruction and
   width8/log6..9 primitive evidence. Preserve source attribution and failures.
4. Add plain checked DoubleFast plan and long-key layout constructor. Test all
   four source tiers, cap/error precedence, checked per-table/sum layouts,
   minMatch dispatch and target/search non-consumption. No public capability yet.
5. Assemble two existing table owners and parser source/cursor records. Test
   distinct/borrowed owners, missing/mismatched layouts, both construction
   failures, reserve-before-mutation and reset lifetimes; no new unsafe feature.
6. Implement the safe single-block DoubleFast parser against the frozen direct
   records. Compare exact literal bytes, LL/ML/distance/offBase, full wire replay
   and both full sampled tables; run safe/relevant-feature/no_std/Miri checks.
7. Capture ordinary-frame trajectories in a separate reviewed unchanged-C helper,
   and optionally collector corroboration separately. These can run in parallel
   with step6 once their source recipes/expected facts are frozen and reviewers
   have independent ownership. Do not use Rust output as their expected input.
8. Add persistent two-table state with exact source/block bounds and poison
   semantics; compare it against ordinary publication fixtures, preserving raw
   versus semantic repeat distinctions and no per-block reseeding.
9. Split shared integration into three independently committed checkpoints:
   common plain frame settings/header, explicit entropy policy, then a small
   static search/emission dispatch. Compare all nine frozen Fast benchmark
   frames and exact allocation counters after each, alongside the seven named
   baselines. Preserve clone/reserve order, and avoid a broad generic framework
   or an inactive algorithm's allocations. These preparations can precede step8
   without enabling DoubleFast.
10. Add whole-buffer DoubleFast emission, truthful window21 headers, first/raw/RLE
    gates, all actual failure ordinals and original/borrowed output ownership.
11. Extend existing public dispatch/admission and documentation only now. Update
    former-unsupported tests deliberately; assert level0/request-default/level3
    equivalence, tier-dependent level2/4 behavior, known-hint precedence and
    independent overrides. Keep named defaults unchanged.
12. Port newly admitted official assertion subsets and replay the saved selector
    corpus with explicit old/new admitted counts. Add bounded bidirectional C
    interop; retain all context, destination-capacity, dictionary and worker gaps.
13. Run focused i686/s390x interpreted ownership/bounds cases, thumb no-atomics
    compile/codegen and the complete feature matrix on an immutable snapshot.
    Keep interpreted/compile-only versus native execution explicit.
14. Prepare/validate deterministic performance workloads before timing. Compare
    safe, unsafe-match-table alone, other individual relevant backends and all
    features with identical dimensions/source/output checks; report table counts,
    actual owner/capacity scope and complete samples. Include Fast baselines and
    independently measured DoubleFast output sizes, with no C ratio/throughput
    parity or universal speedup claim. Time only after a coordinator CPU barrier.
15. Reconcile exact implemented assertions into inventory, preserve unsupported
    obligations, archive validation and clean the completed module checklist.

Do not combine oracle capture, parser implementation, shared entropy refactor
and public admission into one commit. Plain records, fixtures and the shared
policy extraction are separable; live ownership of shared files must remain
explicit if parallelized.

## Migration compatibility of frozen observations

Keep all 81 frozen `reference-fast-fuzz` C selector tuples, source bytes,
capacity observations, provenance and hashes unchanged. Their
`expected_rust_classifications` field records **Module12 admission policy**, not
an eternal C fact. Adding DoubleFast requires a separately versioned successor
policy adapter/checker or qualified expectation layer. Do not regenerate the C
selector bytes or change their source consumption to fit the newer API.
Retain the historical Module12 counts and evidence alongside the newly executed
successor counts. Test both the immutable C selector identity and the current
policy classification independently.

The same migration applies to the 92-case public interop campaign: its old
negative expected statuses include formerly unimplemented DoubleFast profiles.
Update successor live-test expectations deliberately and capture any new C
positive cases as an additive, independently validated campaign. Do not relabel
historical error records as successful executions. Admitting level0/3 and the
level2 gap does not turn every old unsupported/resource error into success;
re-resolve each exact request and apply the selected resource policy.

Preserve all seven named default-frame baselines, existing Fast frame
fingerprints and Fast allocator operation order. The new long-key constructor
must support width8 at private low hash logs without altering either existing
Fast width validation or the named-policy layout domain.

## Official tests unlocked, and obligations that remain

- `tests/fuzzer.c:1554-1565`: empty input levels -1..22. Existing whole-buffer
  admitted subset can expand to -1,0,1,2,3 (5 of24); 19 remain unsupported at
  known-empty size. A fresh request is still not the original reused CCtx setter
  and caller-buffer contract. Empty success alone is not evidence the parser ran.
- `tests/fuzzer.c:1637-1665` and its continuation: original 30-byte
  initial-match source can expand to levels1/2/3 (3 of19; 16 remain unsupported
  at that size). Preserve the exact bytes0..19 followed by0..9. Live reuse,
  `compressBound-1` streaming-forcing and actual streaming branches remain gaps.
- `tests/fuzz/simple_round_trip.c:52-100,:141-179`: exact saved selector order
  and original-length capacity observation are reusable. The current81-record
  selector corpus would move from57 admitted/20 unsupported/4 advanced-policy
  records to61/16/4 by adding two30-byte level0/3 cases and two level2 gap cases.
  This count is an inspected forecast, not a completed replay. Preserve explicit
  advanced nonconsumption and context/reduced-destination/in-place gaps.
- `tests/cli-tests/compression/levels.sh:34-38`: underlying default/level0
  equivalence can have a clearly labelled library adaptation, with explicit
  level3 equivalence added from scalar semantics. The CLI, environment variable
  behavior and whole compression-ratio ordering group are not thereby ported;
  the existing named `compress` default must not silently change to level3.
- `tests/roundTripCrash.c:63-78`: XXH32(first min128 bytes)%19 selects0..18;
  more resolved selections become executable. Preserve exact selection rather
  than replacing every source with level3. Its :81-110 worker/CCtxParams/stream
  path remains outside this no-dictionary one-shot module.
- `tests/fuzzer.c:3208-3233`: explicit DoubleFast CDict attachment with
  hashLog25/chainLog25/window25 remains blocked. A second small hash table does
  not port tagged dictionary tables, by-reference attachment or those resources.
- `tests/fuzzer.c:3496-3535`: level2 parameter-order test enables LDM and uses
  live context getters/state and source-specific size/hash equivalence. Adding
  the DoubleFast level2 tier does not satisfy that group.
- `zstd_double_fast.c:17-99,:328-547,:610-775`: dictionary filling, tagged CDict
  matches and external-dictionary crossings need later independent modules.
- Stable/static workspaces, worker pools, LDM, external sequence producers,
  target compressed block sizes, block splitting, advanced random-parameter
  matrices, CLI/corpus infrastructure and higher reference strategies remain
  visible migration obligations. No whole source-file status promotion follows
  solely from adding this algorithm.

The test-subset forecast was independently cross-checked by the fuzzer agent by
source reading only. No original C assertion was executed for this successor
review and no stored Module12 fixture or status was changed.

## Independent design review

The coordinator and a second reviewer checked the no-dictionary loop and its
helpers independently. The final contract corrects a misleading strict-floor
interpretation after reading ZSTD_selectAddr itself: current probes use >=, the
later-long probe uses >, and backward extension may reach equality. It also
separates 7/8/9/10-byte parser cleanup from the <7 ordinary-frame bypass and
records exact within-table store order. No C capture or Rust parser execution
was used as evidence during this review. The shared integration is explicitly
split to preserve existing allocator and output contracts in small commits.
