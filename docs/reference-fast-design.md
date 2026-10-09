# Bounded reference Fast profile execution

Module 12 design, 2026-10-09. Ownership and repeat-state boundaries were
independently reviewed before implementation. This design records no completed
Fast implementation, oracle capture or performance result. The reference is
facebook/zstd v1.5.7, revision `f8745da6ff1ad1e7bab384bd1f9d742439278e99`.
This proposal is deliberately smaller than a complete compression-level API.

## Scope and completion boundary

Implement a distinct reference-derived **Fast parser**, and make it usable
through opt-in whole-buffer level/request compression. Preserve the existing
`compress`, `Encoder`, Greedy/Chain/Lazy, and MatchSearchOptions behavior. Do not
route ReferenceStrategy::Fast to the existing Greedy implementation, or
ReferenceStrategy::Greedy/Lazy to the similarly named independent policies.

First expose actual execution for no-dictionary profiles whose fully resolved
strategy is Fast and whose resource bounds are admitted. This makes level 1 and
negative levels operational across all four source tiers. It also admits level 2
where it actually resolves to Fast; the 128–256 KiB tier resolves level 2 to
DoubleFast and must reject it until that algorithm exists. Level 0 normalizes to
3, which resolves to DoubleFast in every tier, so the initial numeric API must
explicitly reject default level 3 rather than silently substitute an algorithm.
Source: `lib/compress/clevels.h:25–130`, especially rows :28–31, :54–57,
:80–83, and :106–109; `zstd_compress.c:7754–7780`.

This is useful compression, not another inspection-only record. Completion
requires the new parser, real frame emission, exact round trips, reference
sequence observations, level/request entry-point equivalence, allocation-failure
coverage and independent C decoding. It does not require C-identical entropy
choices, compressed bytes, or ratios. Existing adaptive entropy remains an
independent implementation; that qualification must accompany “reference Fast
profile execution.” Dictionary Fast variants, DoubleFast, live numeric setters,
reference chain/row/tree algorithms, LDM and workers remain subsequent modules.

A broader alternative is Fast plus DoubleFast in one module, which would make
levels 1–3 operational across all tiers. That is not the smallest step: DoubleFast
has separate long and short hash tables, distinct selection/lookahead behavior,
and uses `chainLog` for a short-hash table rather than a candidate chain
(`zstd_double_fast.c:110–131`, :176–205). Do not treat that as a trivial Fast
option. Prefer a separately decomposed successor after this Fast foundation.

## Current inputs and incompatibilities

The relevant existing contracts are in README's parameter-inspection/search
sections, TODO's advanced-level roadmap, the Module 10/11 progress entries,
`src/parameters.rs`, `src/parameters/presets.rs`, and the approved parameter and
match-search designs. Inspection already supplies all seven checked fields,
typed Known(0)/Unknown, negative acceleration, explicit overrides, and two-pass
adjustment. Reuse it rather than repeat scalar resolution in a new encoder.

Important differences exposed by the actual code:

1. Existing MatchSearchOptions admits hash logs 10–18. Valid small-source Fast
   adjustment can produce hash logs 6–9. Extend only an internal checked layout
   constructor/admission path for those small tables; keep public named-policy
   bounds and default behavior unchanged. Scalar reference validity is not a
   promise to allocate every log through 30.
2. Fast dispatch maps minMatch 3 to the four-byte implementation; 4/5/6/7 select
   their corresponding keys (`zstd_fast.c:443–477`). It still verifies/emits
   matches of at least four bytes. This is not a universal emitted-match floor
   equal to minMatch. The current width-aware hash primitive is reusable.
3. The current `parse_greedy` resets/copies the table and reseeds retained output
   for every block (`src/encode.rs`, parse_greedy/parse_seeded_chain). Fast must
   retain the table entries produced by its own sampling schedule across blocks.
   Reseeding every retained position would change candidate selection.
4. Fast's no-dictionary search uses a paired-position schedule, not one ordinary
   probe at each byte. Its starting step is `targetLength + !targetLength + 1`,
   at least two, and the gap grows after 128 bytes (`zstd_fast.c:206–348`;
   `zstd_compress_internal.h:32` defines kSearchStrength=8).
5. Repeat-offset matches are probed during parsing. The current parser emits
   logical distances, then PlannedSequence::new chooses the smallest valid wire
   repeat value. That can change the repeat triple relative to Fast's explicit
   versus repeat-offset intent, especially with duplicate offsets. Parser-local
   repeat evolution must not drift from what the emitted sequence section uses.
6. Negative Fast profiles automatically disable literal Huffman compression
   (`zstd_compress_internal.h:685–697`, Fast and positive targetLength). The
   disabled path emits raw literals directly, before literal RLE selection
   (`zstd_compress_literals.c:151–155`). Copying only hash/key/window parameters
   would leave a real profile effect absent.
7. C requires block/literal compression to exceed a minimum gain. For Fast this
   is `(source_size >> 6) + 2` (`zstd_compress_internal.h:673–683`,
   `zstd_compress.c:3032–3035`). Existing named blocks accept any smaller payload.
   Introduce a new-mode-only policy; do not alter named baseline bytes.
8. Existing one-shot headers always use a single segment whose window equals
   input size. A reference profile can select a smaller window. New profile
   entry points must emit a header that truthfully represents the selected
   window, using single-segment only when content size fits that window
   (`zstd_compress.c:4695–4738`). Preserve old convenience APIs unchanged.

## Small public surface and admission contract

The initial public entry points are:

- `compress_at_level(input, level)` and allocator-last `compress_at_level_in`;
- `compress_with_parameters(input, &CompressionParameterRequest)` and its
  allocator-last `_in` counterpart.

The level convenience function constructs a request and delegates to the same
implementation. Resolve with Known(input.len()) and dictionary size zero before
allocation. The existing source hint therefore does not override this known
size. Use the existing ReferenceFrameParameters defaults deliberately for these
new reference-profile entry points (content size and dictionary ID enabled,
checksum disabled), while retaining existing API checksum defaults. If future
frame-option control is needed, add a separate request wrapper rather than
smuggling independent named strategy/window settings into the reference request.

Admission is based on the resolved record, not a fixed list of level numbers:

- strategy must be Fast;
- initial supported window logs 10–20, hash logs 6–18, minMatch 3–7, targetLength
  0–131072; checked conversion and requested storage budget still apply;
- chainLog and searchLog remain validated reference fields but are not invented
  Fast consumers: C Fast does not allocate a chain table
  (`zstd_compress.c:255–262`) and its noDict parser does not read searchLog;
- no dictionary/prefix argument or attachment mode in this first public surface;
- reject other valid strategies with UnsupportedFeature; valid but unsupported
  requested resource sizes with ResourceLimit; invalid scalar input stays
  InvalidParameter. Do not clamp an operational profile to named-policy limits.

These caps admit the unmodified level 1 and negative presets. Larger hash/window
requirements from explicit overrides remain inspectable but not executable.
Level 2 is deliberately input-tier dependent; document it and test the exact
128 KiB / 256 KiB transitions. A future live API can freeze a FrameExecutionPlan
from its pledge/hint at first processing, but that introduces requested/applied
getter and option-precedence decisions not needed for the first useful module.
Keep that as its own subsequent item, not an ambiguous extension of
EncoderOptions.strategy/window_log.

## Data structures and ownership

| Structure | Ownership and invariants | Upstream role |
| --- | --- | --- |
| FastExecutionPlan | Copy record of resolved parameters, checked layout, key width, acceleration, window, block limit, literal/gain policy and requested payload estimate. Admission precedes allocation; no unsafe representation is needed. | cParams plus effective strategy policies |
| FastMatchState<A> | Own one MatchTable<A> and scalar initialized/sampled frontier/frame position. Exact allocator remains with its vector. Reset invalidates entries once per frame; no per-block dense rebuild. Positions are checked absolute usize indices into the borrowed whole input, matching the existing Vec<usize> table and EMPTY sentinel; counters convert to u64 with checked conversion. They are not C pointers or masked u32 aliases. | ZSTD_MatchState_t hash table/window/nextToUpdate |
| BorrowedFastInput<'a> | Borrow original whole input and current block range. Reads validate initialized range, block-end search limit, low prefix bound and match distance. It does not copy or own the source; returned compressed output borrows nothing. | contiguous noDict prefix source |
| FastParseCursor | Scalar anchor, current/next candidate positions, cached buckets/candidates, step and next-step boundary. Every advancement is checked and proves progress, including extreme targetLength and short blocks. | noDict generic search pipeline |
| Explicit offset intent / checked sequence plan | Retain the parser's explicit-distance versus repeat-value selection and validate it against the incoming repeat triple and literal-length-zero rule. Reuse PlannedSequence fields and supplied-table encoder after validation. | ZSTD_storeSeq offBase plus nextCBlock reps |
| PendingFastBlock | Candidate sequence plans, literal bytes and pending repeats; existing allocator-owned buffers may be reused. No entropy-table, literal-tree, or wire-repeat publication until the compressed block is selected and output reservation succeeds; sampled hash writes are separate search history. | nextCBlock versus prevCBlock |

Reusing the existing MatchTable family needs a nonmutating checked lookup and a
separate checked store, or an equivalent narrow operation that can reproduce the
C lookup/write ordering under hash collisions. Its current replace-only API may
not express delayed stores without changing later candidate reads. Keep the
existing equivalent `unsafe-match-table` backend; these are accessor additions,
not a new pointer-heavy data structure or an artificial unsafe parser feature.

C starts valid indices at 2 (`zstd_compress_internal.h:266`, :1334–1341). Normalize
its trace coordinates to Rust source positions explicitly. Do not turn the C
zero-initialized invalid slot into a candidate at Rust byte zero. Existing
checked EMPTY sentinel semantics can remain, with a documented trace mapping.

Whole-input borrowing is particularly useful for this first milestone: the
caller already owns all source bytes, so an admitted full 512 KiB/1 MiB window
can be searched without adding a whole-frame copy or pretending the old 128 KiB
horizon implements it. Bound eligible indices by the actual selected window.
Keep source resolution indexed so a later streaming path can supply a ring
without replacing the parser. Do not add another full codec implementation;
reuse frame/block/literal/FSE emission and allocator owners with a distinct
internal execution plan. Do not call the current BlockEncoder::encode entry
unchanged: it prepares a RingHistory and named-policy search shape. Extract or
route a narrow shared block-emission/entropy stage so borrowed Fast bypasses
named table/chain/ring preparation. Do not allocate an unused ring in addition
to the full caller source, or conceal it from retained-capacity accounting.

The first parser's position domain is usize because every position indexes the
same live caller slice and the reused MatchTable stores usize. Reject arithmetic
or counter-conversion overflow before access; reserve EMPTY=usize::MAX as invalid
and admit only positions strictly below source.len(). Source length/counters use
checked conversions, never truncating casts. Tests near usize::MAX exercise
range/step arithmetic without manufacturing huge slices. There is no new
absolute-u64 table claim in this milestone. A later streaming design must choose
checked conversion/rescaling or a separately designed wider stored-position
representation before accepting absolute counters beyond usize, while preserving
existing named-table footprint and owners.

## Parser and transactional boundaries to verify first

Preserve reference algorithm decisions, using idiomatic control flow rather
than translating goto labels:

- paired probes, exact update order under colliding hashes, short final cleanup,
  acceleration step changes and progress at targetLength 0/1/131072;
- Fast's eight-byte search safety margin even when the selected hash reads fewer
  bytes; exact-width safe reads never justify searching extra C-skipped tails;
- first-position skip, block-end-derived prefix lower bound, and the separate
  repeat validity check. The noDict routine calls getLowestPrefixIndex with
  endIndex at entry, then with current index for repeat availability
  (`zstd_fast.c:207–245`); do not substitute only a generic per-probe distance test;
- repeat probe before ordinary candidate selection, one-byte backward extension,
  general backward extension bounded by anchor/prefix start, immediate zero-LL
  repeat loops, and sampled post-match insertions (`zstd_fast.c:274–289`, :377–419);
- disabled out-of-prefix repeat offsets are local availability state; restoration
  at cleanup must not publish zeros into valid wire repeat state
  (`zstd_fast.c:355–372`).

Fast's parser return array is not itself a complete wire-semantic triple: the
noDict implementation writes only rep[0] and rep[1], leaving rep[2] untouched
(`zstd_fast.c:368–372`). The collector independently reconstructs full repeats
while interpreting offBase (`zstd_compress.c:3469–3492`). Therefore record both
in the C oracle and compare the two parser-used values separately from an
independent full wire replay; do not assert equality of all three raw parser
cells with emitted decoder state. This distinction matters when designing any
future mid-frame strategy changes.

SequenceEncoder currently owns the authoritative committed triple. Add a read
snapshot and a checked explicit-offset preparation path rather than a second
independently committed triple inside FastMatchState. A parser may carry local
candidate repeats, but SequenceEncoder must validate the same succession before
emission. On compressed selection commit tables/tree/repeats together. On raw,
RLE or rejected compressed candidate, discard pending entropy/repeats. Hash
insertions may remain as search history for initialized source that was actually
emitted, but this is separate from decoder-visible repeat state. Oracle-capture
raw/RLE fallback followed by a matching block before claiming the exact C
search-state lifecycle: C may parse before fallback whereas the existing named
path short-circuits uniform RLE blocks.

Allocation failure must drop the unreturned one-shot output and all scratch;
no caller input/request mutation. Every vector uses try_reserve and reserved
Copy pushes. Owned and borrowed non-Clone allocator tests must preserve the
original allocator in returned output, and distinct clone identities must never
free one another's blocks. A later reusable context retains the established
poison/reset contract; do not promise rollback of every already-mutated table.

## Independently verifiable work items

1. **Freeze C parser observations before Rust Fast code.** Separate collection
   with pinned sources/compiler/width and deterministic regeneration. Use actual
   unmodified block/generation entry points and initialized source; freeze
   logical LL/ML/distance plus explicit rep intent, final repeats, block/trailing
   literal boundaries, and selected table snapshots or insertion trace witnesses.
   ZSTD_generateSequences/copyBlockSequences (`zstd_compress.c:3429` onward)
   is at most a secondary observation path: the API is explicitly documented as
   flawed/deprecated (`lib/zstd.h:1596–1602`), and does not alone establish the
   selected block's publication behavior. Prefer a small direct Fast harness
   with independently checked private state setup and actual normal frame
   compression as a separate publication witness. Never infer its safety or
   state equivalence merely because the function is exported. Capture public complete frames as interoperability inputs,
   not as mandatory Rust entropy-byte goldens.
2. **Add plain admission plan.** Reuse existing resolution; admit Fast only,
   prove small adjusted layouts and all checked resource formulas. Test all four
   tiers, Known(0), out-of-domain source size, every negative boundary, level 2
   tier rejection, default3 rejection, strategy overrides, and unused Fast fields.
3. **Add small internal hash layouts and persistent table operations.** Keep
   named public bounds unchanged. Extend safe/unsafe checks for logs6–9, split
   lookup/store collision ordering, reset/reuse, target-width sizes and every
   observed allocation-failure ordinal. Preserve default table fingerprints.
4. **Add checked explicit-offset planning.** Test all repeat/zero-LL branches,
   duplicate repeats, explicit offsets equal to existing repeats, malformed
   intent, and failure/discard/commit. Existing canonical named planning remains
   unchanged. This is a standalone useful sequence-state checkpoint.
5. **Implement safe single-block Fast.** Complete sampling, backward/forward
   matching, repeats, tails and acceleration. Compare frozen C logical traces
   and final candidate state; use small test inputs that force each branch.
   No public numeric admission yet if full frame state is incomplete.
6. **Retain Fast state across blocks.** Cover at least several 128 KiB blocks,
   selected window boundaries, sampled table preservation, and raw/RLE/compressed
   fallback transitions. Use full source borrowing with checked range resolution;
   exercise checked usize boundary arithmetic with small synthetic records
   without claiming a new full-u64 stored-position representation.
7. **Integrate reference Fast literal/gain/header policies.** Emit raw literal
   sections for positive targetLength, use checked Fast gain thresholds,
   and emit honest source/window metadata. Capture the Fast minimum-literals
   threshold too (`zstd_compress_literals.c:112–125`: 64 normally, 6 with a
   valid previous Huffman table); decide explicitly which entropy selection
   differences remain independent rather than claiming complete C block parity.
   Keep named paths bit-identical.
8. **Publish the allocator-aware one-shot entry points.** Original-owner output,
   no borrowed result, all failure ordinals, no_std, output/source lifetime
   assertions and unsupported-profile rejection before allocation. Simple-level
   and request paths must share the same implementation and exact output.
9. **Port applicable official assertions and audit scope.** Run the original
   negative-level simple/advanced equivalence source schedule with an explicit
   corpus provenance; preserve remaining unsupported branches. C-decode all
   selected complete Rust frames, and Rust-decode independent C frames.
10. **Close the bounded Fast milestone.** Safe/each relevant unsafe/all feature
    checks, no_std, Clippy, formatting, narrow Miri on bounds/owners/repeats,
    unchanged named baselines, and a frozen deterministic Fast-specific benchmark
    comparing negative acceleration and safe/unsafe table access. No ratios or
    correctness claims may come from mere level labels or one lucky corpus.

Implementation ownership can be split as follows: C fixture owner; table/admission/parser owner; checked sequence
intent/integration owner; coordinator handles public exports, documentation,
state-policy decisions, baseline evidence and commits. Agree on offset-intent
and borrowed-source APIs before concurrent edits.

## Official assertions and retained gaps

| Pinned source | Proposed treatment |
| --- | --- |
| fuzzer.c:3468–3489 | First meaningful numeric execution port: level -1, source CNBuffSize/5, simple versus advanced compressed-size equality; strengthen to identical Rust frame bytes. Preserve exact inherited CNBuffer via attributed fixture/test-only C generator or explicitly label a selected RDG adaptation. A whole-buffer request API is an adaptation of the advanced live C entry point, not complete CCtx parity. |
| fuzzer.c:1721–1763 | Existing scalar/live named subsets remain. A future live numeric mode can add requested level getters; this Fast-only whole-buffer milestone does not make the original level7 execution available. |
| fuzzer.c:570–586, :680–710, :804–809 | Reuse existing exact parameter assertions as admission inputs; no duplicate claim that inspecting a record runs its algorithm. |
| fuzzer.c:4332–4355 | Existing dictionary profile scalar equalities remain valid; no dictionary Fast execution is added by this proposal. |
| fuzzer.c:3496–3527 | Keep LDM setter-order pair blocked. A no-LDM request-order regression is useful additional Rust coverage, not a port of the LDM assertions. |
| fuzzer.c:3181–3233 | Fast/DoubleFast attached dictionary hashLog25/chainLog25 remains blocked by attachment mode, algorithm and declared resource scope. |
| fuzzer.c:2192 onward, :4357–4393 | Static contexts and estimator monotonicity remain separate contracts; do not substitute TrackingAllocator totals for C estimator results. |
| fuzzer.c:4438–4505 | Original static multi-level index-reduction campaign remains blocked. Small checked-usize boundary tests verify the new whole-slice parser; existing wider counter tests remain separate and neither substitutes for this original campaign. |
| zstreamtest.c:1599–1628 | Numeric11/9 requested-context tests remain partial; source-hint scalar bounds already work. Whole-buffer Known size must ignore hints, with existing oracle rows reused. |
| tests/fuzz/simple_round_trip.c | Add explicit admitted Fast-profile cases to the bounded adapter only after public execution exists; random unsupported strategy/dictionary/worker branches remain partial. |
| tests/cli-tests/compression/levels.sh:14–47 | Library negative-level normalization/equivalence can use the underlying contracts. CLI defaults, environment parsing, ordering ratios, large decimal argument failures and file operations remain unported. |

No complete-migration status promotion follows automatically. Explicitly report
which actual sequence/control assertions are newly executable, which are
adapted, and which still need dictionary/DoubleFast/live-context/LDM support.


## Prerequisite interface specifications

[The C oracle design](reference-fast-oracle-design.md) separates direct parser
state, collector corroboration and ordinary frame publication, including
bounded initialized C arenas and canonical observation schemas. The
[explicit offset design](sequence-offset-intent-design.md) defines the pure
wire transition, one intent-bearing sequence vector, committed-state snapshot
and allocation-preserving preparation path. Their smaller checkpoints are
recorded separately in `TODO.md` before any implementation begins.


## Shared emission and allocator ownership

Extract a private `BlockEntropyEncoder<A>` containing exactly the current
literal encoder, committed/pending Huffman codebooks, sequence encoder and
candidate payload vector. Named `BlockEncoder` keeps its literal/logical-
sequence vectors, tables, seeds, ring, selected handles and counters. Fast owns
one persistent MatchTable, one literal vector, one `SequenceWithOffset` vector
and the shared entropy owner, borrowing the complete source. Fast must bypass
named `encode`/`start_frame` search setup; no duplicate unused ring, chain or
seed allocation is permitted.

For the first extraction, construct existing children in their existing order
and move complete owners into the aggregate. Preserve original allocator
identities, clone order, reserve order, retained capacities and named frame
bytes. The aggregate needs no additional allocator factory. Only later add a
Fast preparation method; named preparation keeps its current canonical plans
and literal selector. The extraction is its own verified work item.

Before entropy work, validate checked reconstructed lengths: `sum(LL)` cannot
exceed the literal buffer, the remainder is trailing literals, and
`literals.len() + sum(ML)` equals the original block size. Byte equality and
window eligibility remain parser responsibilities. Explicit preparation uses
the committed-repeat snapshot specified in the offset interface. Failed
preparation discards pending state and exposes no candidate. Reserve output
for the raw bound before appending a chosen block; publication after Copy
appends is allocation-free. Raw/RLE discard pending entropy/repeats while
sampled source indices remain eligible according to their window.

Fast literal policy must distinguish raw-only from enabled independent Huffman
selection. A positive target length takes raw-only before literal RLE,
histograms or codebook selection (`zstd_compress_literals.c:151–155`). For enabled
Fast compression, minimum gain is `(n >> 6) + 2`, with equality falling back.
The literal test compares streams/jump table/tree description excluding the
literal header; the block test compares the complete literal/sequence payload
excluding the block header. Checked subtraction makes too-small inputs
ineligible. Existing named selection continues using its old size criterion.
C's separate minimum-attempt and Huffman-validity scheduling is not represented
by the existing optional codebook; retain the independent-entropy qualification.

A pure header record for the new known-size, no-dictionary path uses the admitted
window and reference checksum default of false. Single segment applies exactly
when input size fits that window; otherwise emit its descriptor. Block size is
`min(128 KiB, window)`, and empty input emits one final empty raw block. The new
header follows C's four/eight-byte content-size promotion at `0xffffffff`
(`zstd_compress.c:4695–4738`); the existing convenience encoding remains unchanged.
Test large scalar boundaries without allocating those input sizes.

The original allocator moves into returned output. Workspace children receive
explicit fresh clones; every vector retains the exact allocator that owns its
blocks. Borrowing a non-Clone allocator through `&A` remains supported. Admit
strategy/resources and check the raw output bound before allocation. An error
drops all unreturned output and scratch without changing caller input/request.
Reference parser observations establish decisions for the same input and state;
independent entropy can select different block kinds and hence different later
committed repeats. Do not infer universal C frame-byte or block-kind identity.
