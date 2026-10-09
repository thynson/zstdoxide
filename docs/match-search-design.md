# Configurable named match search

Status: approved implementation design for Module 11; checkpoints are tracked in TODO.md.
Repository inspected at ba4529c9e44eee2218209bb660038e42848d11e8, with the completed
Module 10 reset/query work present. Reference: facebook/zstd v1.5.7, commit
f8745da6ff1ad1e7bab384bd1f9d742439278e99.

## Decision and scope

Make the existing Greedy, Chain, and one-byte Lazy policies configurable through
real storage and parser consumers. Preserve every default dimension, seed order,
tie rule, block/entropy publication rule, frame header, reset contract, and default
encoded byte. Keep reference CompressionParameterRequest and
ResolvedCompressionParameters completely separate from these operational options.

Do not add numerical compression levels, Fast/DoubleFast/Lazy2, binary trees,
optimal parsing, row matching, LDM, persistent cross-block indexes, three-byte
matches, thread workers, or a dictionary-search-budget knob. Do not claim C
compression-ratio, candidate-order, or parameter equivalence. The only exact new C
algorithm correspondence in this scope is the unsalted four-through-eight-byte
hash primitive.

The following initial operational limits have explicit storage and work
consequences. They are separate from the reference parameter bounds.

## Public API

Add a new module and public plain record MatchSearchOptions, with private fields,
Clone, Copy, Debug, Eq, PartialEq, and Default. It contains no allocation, pointer,
or borrowed source and has one shared safe representation under every feature.

Recommended signatures:

    impl MatchSearchOptions {
        pub const fn new() -> Self;
        pub fn set_hash_log(&mut self, value: u8) -> Result<()>;
        pub fn set_chain_log(&mut self, value: u8) -> Result<()>;
        pub fn set_key_bytes(&mut self, value: u8) -> Result<()>;
        pub fn set_chain_visits(&mut self, value: u8) -> Result<()>;
        pub fn set_lazy_visits(&mut self, value: u8) -> Result<()>;
        pub fn set_history_bytes(&mut self, value: u32) -> Result<()>;

        pub const fn hash_log(&self) -> u8;
        pub const fn chain_log(&self) -> u8;
        pub const fn key_bytes(&self) -> u8;
        pub const fn chain_visits(&self) -> u8;
        pub const fn lazy_visits(&self) -> u8;
        pub const fn history_bytes(&self) -> u32;
    }

Every setter validates before mutation. Invalid values return InvalidParameter
without changing any field. Zero is invalid, not an automatic-selection sentinel:
these are concrete Rust controls, not the reference's C request parameters.
Separate Chain and Lazy visit fields make their scope explicit; Greedy always
examines its single newest candidate. Neither visit field changes Greedy.

Add these methods to the existing generic Encoder implementation:

    pub fn search_options(&self) -> MatchSearchOptions;
    pub fn set_search_options(&mut self, options: MatchSearchOptions) -> Result<()>;
    pub fn search_retained_capacity(&self) -> usize;

Keep EncoderOptions unchanged. This avoids breaking its existing complete struct
literals and separates frame settings from operational search storage. Constructor
families keep default MatchSearchOptions. Callers can construct a context with any
existing allocator/dictionary constructor, then set search options at the fresh
boundary before any process call. Additional constructor permutations are not
needed in this module.

set_search_options follows set_options: phase-first validation, only Active with
header_started == false, no allocation or allocator clone, no dictionary change,
no counters/buffer mutation, and no poison recovery. Finished is rejected.
Successful installation stores a validated request; actual storage preparation
occurs when the next frame/block first needs that storage.

set_options continues to change only EncoderOptions and preserves search options.
reset and reset_with_options preserve search options; the latter explicitly
replaces frame settings, not the separate search record. reset_parameters and
reset_all restore both option records to their existing defaults, clear seeds and
attachments as now, and retain allocated owners/capacities without reshaping them.
They retain their Module 10 phase and counter semantics. Document these
cross-record rules and test both operation orders.

search_retained_capacity reports actual vector capacity bytes for both Greedy
owners, both Chain owners, and retained output history, including inactive
strategy caches. It excludes literal/sequence/entropy/staging storage, dictionary
payload, shared control blocks, inline handles, and allocator bookkeeping. The
existing retained_capacity method keeps its wider existing meaning, including its
dictionary reachable-capacity qualification. This narrow new counter is a
measurement, not a memory-limit promise.

Existing whole-buffer helpers remain default-policy operations. Configurable
one-shot constructor variants are deferred; a configured Encoder already provides
the supported operation without multiplying public APIs.

## Exact dimensions, effort, and bounds

| Setting | Default | Admitted range | Actual consumer |
| --- | --- | --- | --- |
| hash_log | 16 | 10..=18 | Active initialized buckets for Greedy working/seed tables and Chain working/seed heads |
| chain_log | 18 | 10..=19 | Working Chain cyclic node count, 2^chain_log; shared by Chain and Lazy |
| key_bytes | 4 | 4..=8 | Exact key extraction and corresponding unsalted hash for every seed, insertion, lookup and probe |
| chain_visits | 8 | 1..=64 | Maximum examined links for each Chain probe |
| lazy_visits | 16 | 1..=64 | Maximum examined links for each of Lazy's current and next-position probes |
| history_bytes | 131,072 | 1..=1,048,576 | Retained output horizon, min(requested bytes, advertised frame window) |

The dictionary Chain seed node count is min(working node count, 65,536).
Hash log is common to working and seed heads. No independent seed-size knob is
needed. Exact defaults remain working nodes 262,144, dictionary nodes 65,536, and
65,536 hash buckets. A node count is insertion capacity, not guaranteed retained
source distance. A visit counts an examined link, including future, stale,
out-of-range, and rejected candidates, not only successful resolver results.
Stale or invalid/non-descending tags terminate as they do now.

Lazy remains one-byte lookahead. It inserts the current position before probing
the next position, preserves distance-one opportunities, delays only if the next
match is longer by more than one byte, and retains the current match on ties.
An explicit lazy visit setting must reach both probes. Chain and Lazy retain
longest-then-nearest ordering, not cheapest encoded-sequence ordering.

The emitted-match floor stays four. Key width is a search-key setting: collisions
must still be verified against source bytes, and a wider-key collision can produce
a valid four-through-(width-1)-byte match. Main probing requires a complete selected
key; tails shorter than that key produce literals instead of speculative reads.
A larger key is not minMatch3 support or a guarantee about compression ratio.

Maximum history is one MiB, eight times the existing block/history default.
The wire window remains EncoderOptions.window_log in 10..=31. Blocks remain at
most min(window, 128 KiB). A one-byte history setting is valid; zero is deferred.
A history limit affects retained previous output, not access to already-known
bytes earlier in the current block, so it does not cap all emitted match
distances to history_bytes. Every ordinary match still obeys the wire window.

The existing Greedy dictionary distance ceiling remains (1 << 29) - 4. Greedy
preparation scans the same suffix as before; at width four this may insert up to
536,870,905 starts for a sufficiently large dictionary. This is a bounded but
potentially expensive per-frame cost, and default byte compatibility requires
preserving it. Do not quietly substitute a one-MiB dictionary scan to match the
new output-history ceiling. No new dictionary-start budget is proposed for
Module 11. Chain retains only its bounded final seed starts. For dictionary length D,
key width W and seed node count S, insert regular starts in
`D.saturating_sub(S + W - 1)..=D - W` when D >= W. This yields exactly
min(S, D - W + 1) starts. Both the lower-bound suffix and upper key-end bound
change with width; the existing +3 becomes +(W - 1). Preserve ascending order.
Test S=1/2, W=4/8 and lengths around S + W - 1. Greedy retains its separate
distance-limited start calculation.

Per-block history rebuilding remains bounded by one MiB of starts plus at most
width-1 dictionary boundary starts and 128 KiB of block starts. This can perform
about eight times the default history reinsertion work on large-window inputs.
The 64-link cap bounds candidate visits, not an exact CPU-time or byte-comparison
budget. An input/frame can contain arbitrarily many blocks; no total wall-time
guarantee is made. Persistent indexes should be a separate later design if these
measured costs warrant them.

## Typed internal records and owner accounting

Use an internal immutable SearchShape derived from validated options, selected
strategy, and actual frame window. It contains scalar bucket/node counts, key
descriptor, effective history length, visit effort, and checked requested byte
totals. Resolve shape before cloning allocators, reserving or mutating storage.
An internal empty one-shot frame can have actual window zero; its effective
history is zero and it must remain valid without allocating a ring. Nonempty
streaming frames retain their existing validated window rules.
All shifts, products, sums and allocation layouts are checked. Invalid public
ranges yield InvalidParameter; unrepresentable layout/arithmetic or an internal
admission ceiling yields ResourceLimit; a valid reserve failure yields
AllocationFailed.

A private SearchKeyLayout holds hash_log and key_bytes. A private copied MatchKey
holds the selected width and its little-endian numeric bytes (or an equivalent
eight-byte initialized array). These are plain safe values. The width identity
is carried into leaf operations so an incompatible key is rejected before
mutation; do not accept an untyped bucket index from arbitrary parser code.

Let H = 2^hash_log, N = 2^chain_log, S = min(N,65,536), and R be the effective
history horizon. Let P = size_of::<usize>(), L = size_of::<Link>(), and
T = size_of::<Node>(). The requested initialized payloads are:

| Owner(s) | Payload expression |
| --- | --- |
| Greedy working table | H * P |
| Greedy dictionary seed | H * P |
| Chain working heads and nodes | H * L + N * T |
| Chain dictionary heads and nodes | H * L + S * T |
| Retained output | R |
| All cached search families together | 2*H*P + 2*H*L + (N+S)*T + R |

There are seven vector owners when all search caches have been allocated: two
Greedy vectors, four Chain vectors, and the history vector. There is one working
Chain shared by Chain and Lazy, not two.

For the current 64-bit layout P=8, L=16, T=32:

| Shape | Greedy pair | Chain working | Chain seed | History | All cached search |
| --- | --- | --- | --- | --- | --- |
| Default | 1 MiB | 9 MiB | 3 MiB | 128 KiB | 13.125 MiB (13,762,560 bytes) |
| Every proposed dimension at its ceiling | 4 MiB | 20 MiB | 6 MiB | 1 MiB | 31 MiB (32,505,856 bytes) |

Use size_of in executable calculations and tests, not these 64-bit constants.
Some 32-bit ABIs align u64 to four bytes and others to eight; Link and Node sizes
must not be guessed from pointer width alone. Each requested allocation fits
isize::MAX and Layout validation. Recommend an additional internal 32-MiB
requested-search-payload admission ceiling. Current maximum dimensions fit it;
a later representation change must recompute or explicitly revise the contract.

The all-cache calculation matters even when only one strategy is currently
selected. Configuration history can leave the largest Greedy cache from an
earlier frame together with the largest Chain cache from another frame and the
largest ring ever requested. Because every family's admitted dimensions have a
fixed ceiling, the combined maximum above remains valid for requested capacities
across strategy changes; estimating only the current strategy does not.

This is not a 32-MiB total-encoder, retained-capacity, allocator, or process-memory
limit. Vec reservations may return more capacity than requested. Grow can
temporarily overlap old and new backing allocations. Ordinary allocator Clone,
allocator bookkeeping and Global Arc/Rc control blocks have independent costs.
Never discard caches silently to make an observed metric fit a logical bound.

Other existing reusable owners remain block-bounded and must stay in the wider
retained_capacity measurement:
- input staging and pending output;
- literal bytes, logical sequences and candidate payload;
- LiteralEncoder's bit writer, four-stream bytes and selected section;
- SequenceEncoder's plans, three symbol vectors, section and bit writer;
- three predefined inverse FSE tables plus each channel's current/spare table.
Huffman codebooks are copied inline values. Temporary tree/entropy construction
storage can add live/transient allocation beyond retained capacities. Immutable
dictionary bytes/tables belong to the selected independent D domain and may
remain reachable through a shared handle. The new search admission calculation
does not claim to estimate any of these excluded categories.

## Dynamic leaf APIs and publication

Keep each Vec<A> paired with its exact retained allocator. Resizing an existing
owner must never use a fresh clone to reallocate or deallocate its storage.
Fresh multi-vector construction can use independent A: Clone owners as now;
borrowed &NonCloneAllocator remains supported. No raw-parts transfer or dynamic
dispatch is needed.

Recommended private leaf shapes:

    MatchTable::new_in(layout: SearchKeyLayout, allocator: A) -> Result<Self>
    MatchTable::reconfigure(&mut self, layout: SearchKeyLayout) -> Result<()>
    MatchTable::reset_from<B: Allocator>(&mut self, seed: &MatchTable<B>) -> Result<()>
    MatchTable::replace(&mut self, key: MatchKey, position: usize)
        -> Result<Option<usize>>

    MatchChain::new_in(layout: SearchKeyLayout, nodes: usize, allocator: A)
        -> Result<Self>
    MatchChain::reconfigure(&mut self, layout: SearchKeyLayout, nodes: usize)
        -> Result<()>
    MatchChain::reset_from<B: Allocator>(&mut self, seed: &MatchChain<B>)
        -> Result<()>

Construction wrappers preserving today's private defaults may remain during
incremental migration. A successful reconfigure starts empty storage at the new
active shape; it is not a resize of a live candidate set. Reconfiguration is
between frames/blocks at the owning context's defined reset/preparation boundary.

For multi-vector reconfiguration, validate the full shape and reserve every
needed vector before changing initialized lengths, layout metadata, links, tags,
next_slot or last_tag. If the first reserve succeeds and a later reserve fails,
the old logical state and old initialized elements remain valid, while capacity
and pointer identity may have changed. This is logical atomicity, not allocation
rollback. After every reserve succeeds, truncate/fill and push initialized Copy
values only within reserved capacity, then publish the new metadata. Use
try_reserve_exact(target_len - old_len) only when growth requires it; do not
confuse capacity-relative and length-relative reserve arguments.

MatchTable seed copy first requires equal layout and active initialized length.
MatchChain additionally requires seed node count <= working node count. Validate
every condition before any copy or reset. A mismatch returns InvalidParameter,
rather than reaching copy_from_slice's panic or accepting hashes made with a
different width. Preserve current chain copy ordering: copied seed slots retain
their tags, and a larger working ring begins insertion after the seed's active
node prefix; an equal-size ring preserves the seed's cyclic next slot.

Rebuild selected dictionary seeds for each prepared frame, in the same ascending
position order as the default path. Retained allocations do not validate stale
content/layout. Empty or short dictionaries leave no live old seed. An inactive
cache may retain a different old layout, but it must be reconfigured and cleared
before reuse. A live candidate iterator's borrow excludes reshape/reset.

BlockEncoder is not globally allocation-transactional. Selected seed preparation
can succeed before another seed/entropy/history allocation fails. Preserve the
current process failure/poison/reset contract and do not promise rollback of all
scratch capacities or prior seed state. Only fully encoded blocks publish
pending Huffman/FSE/repeat state and advance RingHistory. If preparation fails,
later processing is rejected until reset; reset retains every surviving exact
allocator owner and permits a fresh valid configuration.

The public option setters remain allocation-free despite deferred storage
failure. Test the distinction explicitly: valid option installation can succeed
with a failure injection armed; the next actual allocation may fail and poison
process. Reject incompatible phases before inspecting/changing options.

## History and virtual-source details

Construct the encoder ring with the module's one-MiB permitted maximum, while
continuing to reserve only the selected effective horizon through start_frame.
No change is needed to the decoder's maxima or public resource policy.
RingHistory already preserves its exact owner through grow/shrink/regrow.

A larger horizon must not introduce an owning pointer to input or dictionary.
Keep MatchSource as a temporary immutable dictionary/ring/current-block view.
Keep frame counters in u64 and check D+H+local coordinates in usize. At 32 bits,
a very large dictionary plus history/current input may be unrepresentable even
when each range individually exists; reject it with ResourceLimit before index
arithmetic or conversion. Do not silently truncate dictionary coordinates.
Every returned candidate still passes the actual source/window/retention checks.

Update boundary extraction to accept the selected width and return
Result<Option<MatchKey>>: None means an otherwise valid tail lacks enough
initialized bytes for a complete key. It is not corrupt input. Err remains for
invalid state/coordinate arithmetic. This matters for small Flush blocks and
width eight with fewer than seven bytes after a dictionary/history boundary.

Dictionary boundary seeding considers at most width-1 starts, and only while
frame byte zero is still retained. History seeding includes starts that cross
into current input only when their full key exists. Never manufacture zeros
for missing source bytes or pad a logical source to permit a lookup.

A shorter search horizon can evict frame byte zero while dictionaries are still
eligible under the larger wire window. Preserve dictionary-only matches and
clip dictionary-to-frame crossing when its required frame prefix is absent.
Dictionary eligibility remains produced <= window, with distinct tests at
window-1/window/window+1. A dictionary's valid distance can exceed the ordinary
window; applying the history horizon or window distance rule to it would be a
format regression.

## Hash primitives and independently pinned fixtures

Use unchanged upstream zstd_compress_internal.h:898-944 as the reference for
ZSTD_hash4/5/6/7/8 and their zero-salt pointer wrappers. Implement the arithmetic
idiomatically with explicit wrapping multiplication and little-endian loading:

- four bytes: existing 32-bit multiplier 2654435761, high hash_log bits;
- five/six/seven bytes: low 40/48/56 source bits shifted left 24/16/8, multiplied
  by the corresponding pinned 64-bit prime, then high hash_log bits;
- eight bytes: full 64-bit input with the pinned 64-bit multiplier.
- No salts, hash3, row tags, or CPU-specific overread behavior are introduced.

The Rust extractor reads exactly key_bytes initialized bytes into a fully
initialized value. A five-byte caller slice must work without eight readable
bytes. The C oracle may give its pointer wrappers eight initialized bytes, but
must vary the unused high bytes for widths five through seven to show that those
bytes do not affect the primitive. Do not imitate C's physical eight-byte read.

Before changing consumers, add a tiny bounded test-only C helper linked against
the pinned source outside the repository. Save attributed vectors with source
revision/helper/source hashes. Suggested deterministic Cartesian vector set:
five widths, nine admitted hash logs, and 64 fixed eight-byte patterns, for 2,880
primitive records, including zero, all ones, single bits, asymmetric byte order,
incrementing patterns, and deterministic pseudorandom values. Add independent
unused-high-byte pairs for widths five through seven and exact width-four
baseline comparisons. The Rust normal tests consume stored vectors; an ignored
live oracle reproduces them. Oracle fixture/source hashes enter the existing
auditor rather than becoming production C dependencies.

Hash conformance is exact; parser output identity with C is not claimed.

## Existing unsafe features and invariants

No new unsafe feature is needed: extend unsafe-match-table and unsafe-match-chain
for their same owned structures. Search options, shape, key and hash arithmetic
remain shared safe code. Existing unsafe-ring-history/dictionary/source paths
retain their own checked contracts.

For MatchTable, replace the fixed HASH_BITS16 proof with:
1. checked layout and live initialized-length invariants;
2. runtime key-width compatibility and bucket < positions.len checks before
   access in both safe and unsafe builds;
3. valid/aligned/initialized allocation owned by its exact A;
4. shared or exclusive borrow rules preventing resize/aliasing during access.
Capacity alone, debug_assert or a validated public request is insufficient.

For MatchChain, check layout compatibility and active head/node bounds before
copy/access, retaining tag equality, descending links, finite examined-link
budget and u64 tag exhaustion. The iterator's reference must prevent storage
reconfiguration. Increase its admitted visit ceiling to 64 without changing
what consumes a visit. An exhausted stale link still terminates safely.

Update each nearby SAFETY proof to mention dynamic initialized prefixes and
reconfiguration. Avoid introducing unchecked key loads: the hash primitive gains
nothing from a new pointer-access family. Run every changed unsafe backend on
the same malformed-index, mismatch, wrap and allocation-owner vectors.

## Default compatibility and observable controls

Before source changes, capture default frames for all three named strategies
using the current committed implementation. Include multi-block sources,
128-KiB retained boundary cases at wire windows 17/18, raw/formatted dictionaries,
dictionary sizes around 65,536+3, short Flush boundaries, entropy new/repeat
states, fragmented output, and Greedy/Chain/Lazy/Greedy reuse of one context.
Preserve existing module baselines, official sequence sweep hashes, allocator
bench fingerprints and ordinary one-shot defaults. Store the new small baseline
outside implementation generation and verify independent pinned-C decoding.

Default options must reproduce all those bytes across safe and unsafe builds;
capacity changes caused solely by lazy maximum metadata are not justification
for output changes. The new ring maximum must not reserve one MiB for defaults.

For each nondefault knob, require a concrete consumer witness:
- hash size: different initialized count plus a constructed collision/candidate
  selection witness, not only a getter;
- chain nodes: cyclic eviction/order witness plus public input whose available
  earlier candidate differs;
- visit limits: trace exact examined links, including rejected/stale/future
  links, and prove both Lazy probes receive the requested effort;
- width: exact C hash vectors and default/nondefault candidate changes at tails
  and dictionary/history crossings;
- history: public cross-block matches just inside/at/outside the selected horizon
  and independently inside/outside the wire window.

Different valid settings may legitimately emit identical bytes for some inputs.
Do not demand a difference on arbitrary data or substitute metadata for a
designed actual-effect witness.

## Ordered small commits and ownership

Each item should be recorded in TODO before implementation and committed only
after its focused checks; the coordinator owns commits/shared docs.

1. Freeze approved API, ownership/resource formulas and the independent default
   baseline. No live no-op controls are exposed.
2. Add the plain validated options, key-layout/shape records, checked payload
   calculation and platform-bound tests. Keep live Encoder setter unexported
   until consumers exist.
3. Add the shared safe width-four-through-eight hash/extraction layer, attributed
   stored C vectors and ignored reproduction helper. Preserve default hash bytes.
4. Make MatchTable dimensions/layout dynamic, with default wrappers, checked
   reconfigure/seed-copy/key compatibility, allocation-failure tests, and updated
   equivalent safe/unsafe access proofs. Keep a complete working feature matrix
   in this commit rather than landing a broken unsafe path.
5. Make MatchChain dimensions/layout dynamic with the same ownership/failure
   guarantees; preserve seed transfer and cyclic tags. Cover tiny private node
   counts as adversarial leaf tests even though public chain_log starts at ten.
6. Parameterize parser width and explicit Chain/Lazy effort. Freeze source order,
   lookahead, tie rules, exact visit accounting, output reserve/prevalidation and
   partial resolver-error behavior.
7. Thread frame-local shape into BlockEncoder working and dictionary seeds;
   resize/rebuild only required owners, validate compatibility, and preserve
   entropy commit/discard. Existing Global wrappers use exact defaults.
8. Expand selected ring horizon and width-aware stitched-source extraction.
   Verify short-block None tails, ring wrap, source coordinates and dictionary
   eligibility/crossing. Keep decoder untouched.
9. Expose Encoder search setter/getters/capacity and finish reset/debug/docs
   integration. If earlier integration needs an internal setter, keep it private
   until all live consumers are complete.
10. Add independent public conformance/default/actual-effect witnesses and every
    observed allocation-failure ordinal for representative grow, shrink, regrow,
    strategy switch and dictionary cases. Include distinct owned Clone domains
    and borrowed NonClone owners; preserve valid logical state on leaf failure
    while allowing retained-capacity growth.
11. Focused safety/platform/interoperability validation, complete fixture audits,
    then the full safe/individual/all/no_std matrix. Use small Miri tests for
    dynamic bounds/seed mismatch/owner failure; native tests cover maximum
    dimensions and broader campaigns. 32-bit checks verify layout arithmetic and
    virtual-coordinate rejection; hash fixtures cover endian independence.
12. After all source/checks are frozen and CPU is quiet, benchmark identical
    workloads in default and a small documented tuned grid, across all existing
    backend builds. Separate backend access effects from larger storage/search
    effort. Record encoded size/full fingerprints, timing samples/ranges,
    allocation counts/peaks, search and total retained capacities. Include
    history-rebuild overhead. No universal speedup or C throughput claim.

After the hash/key interface is frozen, MatchTable and MatchChain leaf owners can
work independently. The parser owner can prepare traces/tests against that
agreed interface, but should wait for the chain signature freeze before edits.
The independent baseline/C-fixture owner can work in separate test/oracle files.
BlockEncoder, stitched source, and live Encoder integration should have one
coordinating owner or sequential checkpoints because they share source/state.
Do not let separate agents reshape the same context files concurrently.

Implementation refinement: the borrowed dictionary/history/input key-extraction
work is verified as its own small checkpoint after parser probes and before
frame-local storage integration. It first establishes checked coordinates and
incomplete-tail behavior while the existing four-byte default consumers remain
active. Storage integration then selects those helpers, and the following
history checkpoint changes the retained horizon. This keeps source-boundary
correctness separately reviewable from allocation and context configuration.

The benchmark work is also split into a clock-free harness checkpoint and a
separate final timed campaign. The harness is validated before the complete
immutable feature matrix, so measurements add no unverified implementation.
See [the benchmark design](match-search-benchmark-design.md) for deterministic
profiles, workloads, accounting scope and sampling rules.

## Acceptance boundary and remaining obligations

Module 11 is complete when each accepted option controls its actual existing
consumer, defaults remain byte-identical, all bounds and ownership/failure
contracts are tested, and tuned frames decode with pinned C. It is not complete
merely because setters and resource getters exist.

The reference strategy/profile inspection from Module 10 remains inspection
only. Operational profile admission, Fast/DoubleFast/Lazy2/tree/optimal parsing,
persistent indexes, minMatch3, row/dedicated-dictionary mechanisms, LDM, worker
jobs, static workspaces, exact C estimators, and complete upstream-suite migration
remain explicitly open. The C chain's position-masked links and attempts
2^searchLog differ from this insertion-tagged bounded Rust representation;
similar option names do not erase that distinction.
