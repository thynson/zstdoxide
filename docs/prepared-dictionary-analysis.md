# Compiled-dictionary loading analysis

Module 37 item 1 records what the pinned reference owns in a compiled
dictionary, how that state is built and attached, and which Rust counterparts
already exist. Every claim below points at the pinned sources; no Rust
implementation is proposed beyond the ownership decisions this analysis fixes.
The pinned revision is facebook/zstd v1.5.7
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`, inspected in the external checkout
`/private/tmp/module24-preparation/zstd-git` (a fresh pinned fetch; the
original study checkout's Git metadata was pruned).

## What a native CDict owns

`struct ZSTD_CDict_s` (`zstd_compress.c:80-97`) owns the dictionary content
pointer and size plus the content type the CDict was created with, an entropy
workspace, one `ZSTD_cwksp` workspace, one complete `ZSTD_MatchState_t`, one
`ZSTD_compressedBlockState_t` (the prepared previous-block entropy), the custom
allocator record, the dictionary ID, the compression level sentinel and the
resolved row-match-finder switch. Creation reserves one workspace for all of it
(`ZSTD_createCDict_advanced_internal`, `zstd_compress.c:5605-5641`), so the
tables and the entropy state are one allocation domain with one lifetime.

## Creation pipeline

`ZSTD_createCDict_advanced2` (`zstd_compress.c:5660-5707`) resolves parameters
through `ZSTD_getCParamsFromCCtxParams` in `ZSTD_cpm_createCDict` mode, which
applies dictionary-size-tiered presets and the tagged-indices caps:
Fast/DoubleFast hash logs are capped at `32 - ZSTD_SHORT_CACHE_TAG_BITS` when
the CDict indices are tagged (`zstd_compress.c:1575-1580`,
`ZSTD_CDictIndicesAreTagged` at `:299-302`, tag width in
`zstd_compress_internal.h:1482`), and row hash logs at
`32 - ZSTD_ROW_HASH_TAG_BITS` (`zstd_compress.c:1600`). The row-match-finder
switch is resolved once at creation (`ZSTD_resolveRowMatchFinderMode`) and
frozen in the CDict. Dedicated dictionary search exists but stays outside this
module.

`ZSTD_initCDict_internal` (`zstd_compress.c:5551-5604`) first resets the
owned match state for CDict use (`ZSTD_resetTarget_CDict`), then loads the
dictionary once through `ZSTD_compress_insertDictionary` with
`ZSTD_tfp_forCDict` and stores the resulting ID.

## Loading pipeline

`ZSTD_compress_insertDictionary` dispatches by content type. A formatted
dictionary runs `ZSTD_loadZstdDictionary`/`ZSTD_loadCEntropy`
(`zstd_compress.c:5061-5215`): the literal CTable is read with
`HUF_repeat_check` promoted to `HUF_repeat_valid` only when every symbol has a
nonzero weight and all 256 symbols are present; the three FSE tables are built
into the owned previous-block state; the repeat triple is read and validated
nonzero and at most the content size; and the offset table's repeat mode
requires every offset up to content size + 128 KiB to be representable. The
content is then loaded into the match state.

`ZSTD_loadDictionaryContent` (`zstd_compress.c:4898-4990`) applies three
ordered caps before any table fill: dictionary indices may reach at most
`ZSTD_CURRENT_MAX - ZSTD_WINDOW_START_INDEX`; tagged short-cache indices
truncate the dictionary to `(1 << (32 - 8)) - ZSTD_WINDOW_START_INDEX`; and the
table-indexable range caps at `1 << min(max(hashLog + 3, chainLog + 1), 31)`.
Only the surviving suffix is loaded. It sets `nextToUpdate` to the loaded start
(the sampling frontier) and `loadedDictEnd` to the loaded end (the expiry
anchor), runs `ZSTD_overflowCorrectIfNeeded`, and fills exactly the selected
finder: `ZSTD_fillHashTable`, `ZSTD_fillDoubleHashTable`, chain insert, row
fill with the context salt, or binary-tree fill. Loading may be skipped
entirely for content at or below `HASH_READ_SIZE`.

## Attach and use pipeline

`ZSTD_compressBegin_internal` (`zstd_compress.c:5238-5285`) never uses both a
raw dictionary and a CDict. A CDict with nonzero content is reused as prepared
state only when the pledged source is unknown, below 128 KiB
(`ZSTD_USE_CDICT_PARAMS_SRCSIZE_CUTOFF`, `:5232`), or below six times the
dictionary content (`ZSTD_USE_CDICT_PARAMS_DICTSIZE_MULTIPLIER`, `:5233`);
then `ZSTD_resetCCtx_usingCDict` attaches or copies the prepared match state
and entropy. Otherwise the CCtx is reset for its own parameters and the same
`ZSTD_compress_insertDictionary` runs again with `ZSTD_tfp_forCCtx` — the
native library re-loads per call in that regime. `ZSTD_checkDictValidity`
(`zstd_compress.c:4629`) later expires loaded content against the resolved
window as blocks advance.

## Rust counterparts already in place

Module 36 resolves the same scalar profile against the source and the
dictionary's matchable content length, stages the in-window dictionary suffix
before the input, seeds the selected per-call tables from that staged region
(`GreedyTables::seed_prefix`, `DoubleFastTables::seed_prefix`, the Fast
owner's seeding and the tree's unsorted-insert back-fill), bounds every
candidate with the resolved-window floor, reuses prepared formatted-dictionary
entropy as the first block's previous state, and serializes the dictionary ID.
The row owner seeds under the same fresh salt every later lookup uses
(`FRESH_ROW_SALT`), which is the Rust counterpart of the native row salt and
short-cache tags: tags written at construction must be readable at search time
by construction, not by a shared mutable salt. Admission rejects every
Optimal/Ultra/Ultra2 profile for nonzero dictionary content.

## Ownership decisions fixed by this analysis

A `CompiledDictionary<'a, A>` borrows immutable dictionary content for `'a`
(the `ZSTD_dlm_byRef` shape) and owns everything else in allocator `A`: the
resolved scalar profile, the per-profile seed tables from the existing
per-call table families, prepared entropy only for formatted dictionaries, the
dictionary ID and the loaded-suffix boundary. Construction validates admission
and reserves every table before publication; failure releases the partial
state. `Arc<CompiledDictionary>` shares without cloning tables or content. The
compiled profile is fixed at construction; a later call whose request resolves
to a different profile must be rejected explicitly rather than silently
reusing mismatched tables or re-forking the profile. Window expiry keeps the
Module 36 candidate-floor rule against the compiled loaded end; the native
cutoffs at `zstd_compress.c:5232-5233` are a native reuse-policy boundary and
not a Rust contract.

The first implementation target stays exact byte equality with the Module 36
combined-staging path for the admitted profiles on deterministic
dictionary/source pairs: seeding tables with the same positions the staged
scan would visit must not change emitted candidates. Where equality cannot be
established for a family, that family stays on the existing per-call path and
the difference is measured, never silently shipped.

## Reuse geometry refinement

Module 36 resets and re-seeds per-call tables from the staged suffix on every
call (`fast_state.rs` `from_parts_with_prefix` resets and seeds;
`GreedyTables::seed_prefix` seeds chain and row owners; the tree back-fills at
its first query). That per-call table work is exactly what a compiled state
removes, and the pinned library has two reuse modes for it: attach, where
`ms->dictMatchState` points at the read-only CDict match state
(`zstd_compress.c:2364`) and every strategy grows dedicated
`dictMatchState` search variants (`zstd_compress.c:3094-3095`,
`zstd_lazy.c:173`), with tagged Fast/DoubleFast indices keeping shared entries
out of context writes; and copy, where the CDict tables are duplicated into the
context workspace. Neither mode promises byte identity with re-loading the same
dictionary per call.

The Rust tables store absolute positions in the borrowed slice and parsers
read candidates through that slice (`seed_chain`/`seed_row` insert `position`;
`parse_next` reads `self.source[position..]`). A compiled table seeded over
dictionary content therefore holds content-relative positions while the
call-time combined borrow contains only the in-window suffix. Three reuse
shapes exist: translate every dictionary candidate position at use by
`delta = content_len - staged_len` with a floor at `delta`; stage `delta`
leading bytes so content positions coincide with combined positions (an
unbounded copy, rejected); or consult the compiled tables as a second read-only
candidate source beside per-call tables that alone receive input sampling —
the `dictMatchState` shape.

Decision: the compiled tables stay immutable during calls and dictionary
candidates are translated at use as a second read-only candidate source; input
sampling mutates only per-call tables. A compiled state can therefore be
borrowed immutably and shared, and per-call scratch (staging, literals,
sequences, per-call tables) stays per-call. The single-table candidate merge of
Module 36 is not reproduced by two candidate streams, so byte equality with the
combined-staging path is a measured comparison per family rather than a
contract; raw and empty dictionaries never build compiled tables and keep the
exact dictionary-free parameterized bytes. A translated dictionary candidate at
content position `p` is usable only when `p >= delta` (its bytes are staged)
and the resolved window bounds hold; the tree frontier and chain/row sampling
frontiers carry over per family.

## Open questions for the implementation items

- The per-family candidate merge order for the second read-only source and
  which of Fast, DoubleFast, chain, row and tree need dedicated search variants
  versus shared translated probes.
- The public error surface when a call's request resolves away from the
  compiled profile, versus a documented per-call fallback.
- Coordinator selection semantics for compiled handles beside the existing
  `DictionaryHandle` selection without cloning tables.
- Allocation-failure ordering for construction when tables and entropy are
  reserved in one domain but validated in stages.

## Fast and DoubleFast consultation (item 3)

The item-3 implementation fixes the candidate-merge question for the two hash
families and measures the resulting geometry instead of asserting byte identity:

- A call stages exactly the compiled `loaded_len` suffix before its input, so
  compiled positions are already combined-space positions and the translation
  delta is zero. The delta and its floor stay unimplemented until a call can
  stage less than the compiled suffix; the state constructor rejects a layout
  mismatch before resetting its table, and the parser still verifies every
  candidate's bytes, offset, block bounds and window.
- `MatchTableView<'a>` is the immutable borrow of one sampled array plus its
  checked layout; `MatchTable::view` and `DoubleFastTables::view` produce it and
  expose no write path. `StagedMatchView<'a>` pairs that view with the exact
  staged-prefix length its positions index; Fast carries one and DoubleFast a
  long/short pair in their per-call states, and both constructors reject a call
  whose prefix length differs from the compiled suffix.
- The merged read is "per-call entry when present, compiled entry otherwise".
  Because the per-call table starts empty and every sample is stored there, this
  reproduces a table seeded with dictionary positions and then overwritten by
  call sampling: a later store always wins. Per-call input sampling therefore
  stays separate and mutates only per-call cells.
- Unit tests compare every block of a compiled-consultation state against
  `from_parts_with_prefix` on the identical combined borrow for Fast and
  DoubleFast. They also prove that at least one emitted sequence had a candidate
  before the frame start (a dictionary hit), that compiled cells were unchanged
  after the call, and that every nonempty per-call cell indexes the frame
  region. These are measured comparisons for these deterministic pairs, not a
  general equality contract. A further case caps the loaded suffix with an
  explicit window and runs three blocks: when the first block fills the parallel
  block limit, the block-end window floor reaches the prefix and no dictionary
  candidate is admissible, while a shorter first block leaves part of the capped
  suffix inside the floor and does reach those candidates.
- A formatted dictionary's prepared entropy is borrowed as
  `DictionaryEntropyView` and its twelve-bit first-block literal codebook is
  built once at construction. The combined path rebuilds that codebook per call;
  the test compares the compiled table with the exact combined-path
  construction. `first_block_seed` exposes the pair the frame workspace consumes;
  consuming it in an emitting workspace, the Greedy and tree families, and the
  public surface remain later items.

## Dictionary chain and row search order (analysis)

The pinned lazy/row finders answer the merge-order question for the next two
families: neither merges candidates by position. `ZSTD_HcFindBestMatch`
(`zstd_lazy.c:667-782`) first runs the context chain through
`ZSTD_insertAndFindFirstIndex_internal` and exhausts it or its
`1 << searchLog` attempts. Only while `nbAttempts > 0` does it enter the
`ZSTD_dictMatchState` branch (`:738-770`): that branch starts from the
dictionary's hash head, follows the dictionary chain with
`dmsMinChain = dmsSize > dmsChainSize ? dmsSize - dmsChainSize : 0`, and maps
each dictionary index into context space by
`dmsIndexDelta = dictLimit - dmsSize`, emitting
`curr - (matchIndex + dmsIndexDelta)`. `ZSTD_RowFindBestMatch`
(`zstd_lazy.c:1141-1330`) uses the same order: the context row collects at most
`nbAttempts` tagged candidates, then the dictionary row (`:1286-1320`) collects
its tagged candidates with the same remaining budget and translates positions
by the same delta. The attempt counter is shared and decremented per collected
candidate, not per family. The dictionary owner is read-only in both branches;
`dms->hashTable`, `chainTable` and the row tables are never written.

In our geometry a call stages exactly the compiled `loaded_len` suffix, so
`dictLimit == dmsSize == loaded_len` and `dmsIndexDelta` is zero. Compiled
candidates already index the combined borrow, exactly as in the Fast and
DoubleFast path, and the per-call window floor still bounds every accepted
candidate. The remaining design decisions are:

- Add allocation-free `ChainView<'a>` (borrowed heads and links, the checked
  key layout and chain log) and `RowView<'a>` (borrowed positions and tags plus
  the row layout). They expose only candidate reads, so no allocator parameter
  enters the parser and compiled cells cannot be reset or inserted.
- `GreedySearch::Chain` carries an optional `ChainView` and
  `GreedySearch::Row` an optional `RowView`. `find_chain` and `find_row` run the
  context candidates first and then the compiled candidates with the remaining
  budget, applying the same `current`, floor, byte and window checks to both.
- The chain order should coincide with combined staging except where a cyclic
  slot was overwritten by a different key during seeding versus context
  sampling. A row is a separate insertion ring in each owner, so sequential
  context-then-compiled order is not the same candidate order as one seeded
  ring. Both families therefore keep byte equality with combined staging as a
  measured comparison (item "compare compiled-state frames") rather than a
  contract.
- Lazy/Lazy2 lookahead queries reuse the same finder, so the compiled stream
  participates per query exactly like the context stream. Repeat offsets stay
  parser-owned and unchanged; the pinned `isDMS` repcode branch
  (`zstd_lazy.c:1537-1600`) is a native two-segment detail with no Rust analogue
  because our combined borrow is contiguous. Dedicated dictionary search, row
  tag-width variants and binary-tree consultation stay outside this analysis.

## Chain consultation (item 6)

The chain implementation follows that order. `ChainCells<'a>` holds the borrowed
heads and links plus the checked key layout and chain log; the owner, the
read-only `ReferenceChainView<'a>` and `ChainCandidates` share it, so the
`unsafe-reference-chain` access has one implementation and no new unsafe
operation. `StagedChainView<'a>` adds the staged-prefix length and is rejected by
`GreedyMatchState` for a mismatched length, a mismatched key layout or a non-chain
owner. `GreedySearch::Chain` carries the optional view into both the direct and
lookahead probes; `find_chain_inclusive` runs the per-call chain first, counts
consumed candidates, and only then starts the compiled chain with the remaining
attempt budget. A compiled candidate still passes the same `current`, floor,
byte and window checks as a per-call one.

Tests compile Greedy, Lazy and Lazy2 at window log 13, which selects the chain
finder, and compare every block against `from_parts_with_prefix` on the same
combined borrow. They also assert at least one compiled candidate reached the
parser, that the compiled head/link arrays are unchanged afterwards, and that
Fast or a row-family Greedy profile rejects the chain accessor. These are
measured comparisons for deterministic pairs; no general byte-identity contract
is claimed.

## Row consultation (item 7)

`RowCells<'a>` now holds the borrowed positions and tags plus the checked row
layout; the owner, `RowTableView<'a>` and `StagedRowView<'a>` share it, so the
`unsafe-row-table` reads have one implementation and no new unsafe operation.
The existing collection gained an explicit limit: the owner passes its layout
attempt cap, and the compiled view receives only the budget the per-call row left
unused. `GreedySearch::Row` carries the optional view into direct and lookahead
probes; `find_row_inclusive` evaluates the per-call snapshot first, then collects
the compiled row with the remaining budget and evaluates those candidates after
it, matching the pinned `dictMatchState` order.

Tests compile Greedy (5), Lazy (6) and Lazy2 (7) with a 32 KiB dictionary, which
tiers to window log 17 and selects the row finder. Every block matches
`from_parts_with_prefix` on the same combined borrow for these deterministic
pairs; the test also asserts a compiled row candidate reached the parser and that
the compiled position/tag arrays are unchanged. The chain accessor and the row
accessor reject each other's profiles and Fast. Because each owner has its own
insertion ring, these are measured comparisons, not a general equality contract.

## Dictionary binary-tree search order (analysis)

The pinned binary-tree finder differs from the chain and row finders in one
structural respect: the dictionary's tree is not a lazily sorted copy of the
context geometry but an eagerly fully sorted tree, and the dictionary search is
a separate read-only descent that can only strictly improve the context result.

Dictionary loading is eager and fully sorted. `ZSTD_loadDictionaryContent`
handles `ZSTD_btlazy2` (with `ZSTD_btopt`/`btultra`/`btultra2`) under the
comment "we want the dictionary table fully sorted"
(`zstd_compress.c:5019-5028`) and fills the dictionary's own match state with
`ZSTD_updateTree(ms, iend-HASH_READ_SIZE, iend)` (`zstd_opt.c:583-588`). Each
insertion runs `ZSTD_insertBt1` (`zstd_opt.c:442-545`), which sorts the new
position immediately into the dictionary tree, uses a window floor computed
from the fill target ("we only need positions that will be in the window at
the end of the tree update"), caps each traversal at `1 << searchLog`
comparisons, and returns a forward skip over repetitive patterns (up to 192
extra positions once a match exceeds 384 bytes), so the update visits fewer
positions than a chain/row fill. Positions within eight bytes of the loaded
end are never inserted (`target = iend - HASH_READ_SIZE`).

The context tree keeps the lazy contract. Per query `ZSTD_updateDUBT` writes
unsorted marks (`zstd_lazy.c:29-65`), and `ZSTD_DUBT_findBestMatch`
(`zstd_lazy.c:281-380`) walks the unsorted chain, batch-sorts it through
`ZSTD_insertDUBT1` (`:106-153`), then descends with path-compressed edge
writes. On an improving full-block match under `ZSTD_dictMatchState` it sets
`nbCompares = 0` so the dictionary search is skipped entirely
(`:349-353`). Only while `nbCompares` remains does the dictionary branch run
(`:369-373`): `ZSTD_DUBT_findBetterDictMatch` (`:155-254`)

- starts from the dictionary's own hash head (`dms->hashTable`, `:161-165`)
  and follows the dictionary's own tree edges (`dms->chainTable`) strictly
  read-only;
- maps dictionary indices into context space by
  `dictIndexDelta = ms->window.lowLimit - dictHighLimit` (`:176`) and bounds
  the walk by the dictionary's own window low and `btLow` (`:174-180`);
- shares the remaining `nbCompares` with the context traversal;
- accepts only strictly better matches under the same gain rule evaluated
  against the carried-in context best, so a tie keeps the context match
  (`:213-219`), and breaks on an improving full-block match (`:221-225`).

`ms->nextToUpdate` is advanced only after both traversals (`:375`). The
dictionary traversal performs no path compression and no sorting, which is
exactly why the dictionary tree must be fully sorted at load: a read-only
descent has no way to resolve an unsorted mark. In our geometry a call stages
exactly the compiled `loaded_len` suffix, so `dictIndexDelta` is zero and the
compiled candidates already index the combined borrow; the per-call window
floor still bounds every accepted candidate.

The remaining decisions for the implementation item are:

- `TreeCells<'a>` (borrowed heads and edges plus the checked layout) is shared
  by the owner, a read-only `ReferenceTreeView<'a>` and the existing checked
  reads, so the `unsafe-reference-tree` access keeps one implementation; the
  view exposes head and edge reads only. `StagedTreeView<'a>` adds the staged
  prefix length and the state constructor rejects a mismatched family, layout
  or staged length before resetting.
- Construction of a compiled tree owner runs the sorted prefix load (the
  `ZSTD_updateTree`/`ZSTD_insertBt1` counterpart) instead of the per-call
  back-fill, because a read-only consultation requires fully sorted edges.
  The per-call tree keeps its first-query back-fill contract with its sampling
  frontier at the combined-buffer start.
- `GreedySearch::Tree` carries the optional view into direct and lookahead
  probes; the search counts the context candidates it compared and consults
  the compiled tree only with the remaining budget, after the context
  traversal, applying the same floor, byte, ordering and window checks. An
  improving full-block context match suppresses the compiled traversal, and a
  compiled candidate that fills the block without improving the maximal
  context best ends the walk because no later candidate can strictly improve.
- Byte equality with combined staging stays a measured per-family comparison:
  the eager fully-sorted load and the forward skip select a different tree
  shape than the per-call lazy back-fill even where a deterministic pair
  coincides. The retained-plan rejection boundary is a Rust contract, not a
  pinned one: `ReferenceSearchPlan::from_resolved` produces the same retained
  plans as `new_with_dictionary`, and both frame paths must keep rejecting a
  nonzero staged prefix for every optimal-family plan with
  `UnsupportedFeature` before any allocation.
