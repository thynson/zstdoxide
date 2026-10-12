# Bounded no-dictionary reference Lazy and Lazy2

This preparation reads facebook/zstd v1.5.7 revision
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`. Module 14's final matrix and timing
remain open. No new C observation, production parser, public admission or speed
result is claimed here. The work items precede implementation in `TODO.md`.

## Scope and ownership

Extend known-size numeric one-shot requests to `ReferenceStrategy::Lazy` and
`Lazy2`. Both use the existing chain/row search families: automatic row selection
requires resolved window log above 14 (`zstd_compress.c:225-250`). The named Lazy
policy remains an independent algorithm. Dictionaries, reusable numeric contexts,
binary trees, optimal parsing, LDM, workers and external sequences stay separate.

Use the existing bounded window/hash limits 21/19 and chain limits 18/8, with
row visits capped at 64 and no artificial cap on its unused chain dimension.
Resolve the actual source once before selecting the family and admitting its
resources. Minimum keys clamp to 4..6; target length does not terminate this
parser. Unsupported strategies still precede resource errors. Fast, DoubleFast
and Greedy retain their existing independent policies and output bytes.

These bounds should admit level 5 across all source tiers once small-source
Lazy is available, and level 6 across all tiers once both depths are available.
Larger levels remain source-dependent: large-source levels 7/8 request hash log
20 and exceed this module's cap, while smaller tiers use admitted layouts.
This is a design prediction from pinned `clevels.h`, to be frozen and verified
by successor source-only mappings before public implementation.

| Role | Ownership and invariants | Upstream counterpart |
| --- | --- | --- |
| Reference lookahead policy | Copied depth value 0/1/2; selected from resolved strategy | `depth` in `ZSTD_compressBlock_lazy_generic` |
| Candidate | Copied source position, previous index, length and offset intent; checked source extents and positive distance | `start`, `matchLength`, `offBase` |
| Gain comparison | Bounded signed value derived from length and positive encoded offset; no allocation or sentinel candidate | `gain1`, `gain2`, `ZSTD_highbit32` |
| Lookahead coordinate | Checked current/anchor/limit scalars; an explicit inclusive query distinct from strict main-loop entry | Preincremented `ip` in depth-one/two search |
| Shared sampling state | Exactly one owned chain or row variant, plus independently owned literal/sequence scratch; complete source borrowed | `ZSTD_MatchState_t`, `SeqStore_t` |
| Publication state | Existing transactional frame/entropy owner and caller-committed wire repeats | Previous/next compressed-block state |

No additional storage leaf is required. Reuse safe reference-chain and row-table
implementations and their two independent opt-in unsafe features. New policy,
score, candidate and cursor records are plain values with a shared safe
representation; artificial unsafe variants add no access optimization. Shared
parser/state composition remains entirely safe and compatible with no_std+alloc.
Each owned Vec must keep its exact allocator value across moves, failures and
publication. No new owner, clone or reservation is justified by lookahead alone.

## Selection and scoring

Review `zstd_lazy.c:1519-1775` and the depth-one/two entry points at 1832/1853 and
1876/1897. Both depths begin by considering repeat1 at current+1, then search the
table at current even when that repeat exists. The explicit match wins only if
strictly longer; a tie keeps the repeat. Greedy depth zero must retain its early
repeat acceptance without performing this extra table search.

At depth one, increment the search position before comparing repeat1 and the
table candidate. Repeat gain is `3 * new_length`; its incumbent threshold is
`3 * old_length - highbit(old_offset_base) + 1`. Explicit gain is
`4 * new_length - highbit(new_offset_base)` against
`4 * old_length - highbit(old_offset_base) + 4`. Only strict improvements win.
An explicit improvement repeats the depth-one search loop, so the policy is not
equivalent to one fixed extra probe.

If depth two is selected and another probe is eligible, increment again. Its
repeat comparison uses multiplier 4 and incumbent penalty 1; its explicit
comparison uses multiplier 4 and incumbent penalty 7. An explicit win again
restarts the depth-one loop. A repeat improvement alone does not take that
continue branch. Store the selected candidate only when further improvement
stops, then perform the existing backward extension and repeat cleanup.

The score's offset base is 1 for repeat1, and distance+3 for an explicit match
(`zstd_compress_internal.h:718-725`). Do not score C's missing-match sentinel or
reproduce unsigned underflow followed by an implementation-defined signed cast.
First require a real match of at least four bytes. With block length at most
131,072 and bounded positive distance, checked signed arithmetic represents
every valid comparison exactly; document that proof beside the helper. Frozen
C observations must cover ties and both sides of each gain threshold.

## Sampling and source bounds

Main-loop entry is strict: current is below end-8 for chains or end-16 for rows.
Lookahead increments after that test and can therefore search exactly at the
limit. Existing `GreedyBlock::search_from` and the direct Greedy search functions
correctly reject equality; preserve that contract and add an explicit checked
lookahead path. Validate the query/key extent, candidate floor and every sampled
index before access. Later bytes from another block cannot rescue a short tail.
Neither a debug assertion nor a supposedly valid candidate replaces a check.

Every speculative search changes tables, next-to-update and row salt entropy,
including rejected candidates. Keep those sampling changes. The skipping flag
persists during lookahead and clears at the existing publication boundary.
Preserve both long-gap rules, per-probe floors, fresh-context salt, newest tie
order, tiny-block bypass and exact initialized table maps. An error after any
sampling mutation poisons the retained context, including future empty calls.
Invalid entry arguments and reservation failures retain the existing earlier
transaction boundaries.

Local repeat saving/restoration, backward extension and immediate repeat2
cleanup remain separate from committed wire repeats. Raw/RLE fallback discards
candidate sequence/entropy publication while retaining sampled history. A
complete-frame test must check these interactions across ordinary schedules;
direct blocks alone cannot establish them.

## Evidence before public admission

First preserve the reviewed complete Greedy clock-free frames and qualified
allocator observations as immutable regressions. Preserve all existing named,
Fast and DoubleFast fixtures and predecessor admission maps. Do not generate
new expected bytes from the refactored parser.

Specify and review a separately versioned direct C packet/helper before its
first execution. It must distinguish depth, branch and source bounds, reject
malformed controls, bound every independent arena and retain the short-input
padding proof. Capture unchanged upstream entry points under normal and fully
instrumented used-unit builds. Keep depth-zero controls that can be compared
with existing observations. Independent replay must validate complete table
maps, literals/sequences, repeat trajectories and reconstructed source bytes.

Recipes must cover initial repeat/table ties, first/second lookahead winners,
multiple successive improvements, gain equality, limit-equality probes, all
row widths, effective key widths, cyclic bias, collisions, long gaps, skipping,
window floors and disabled/reintroduced repeats. Freeze source recipes before
C execution; project observations through a checked, dependency-free Rust test
reader before production parsing consumes expected values.

Then validate ordinary retained-state/publication schedules and complete frames,
all observed allocation failure points, malformed state and exact independent
owners. Public admission requires separate frozen successor maps, original
selector/numeric-loop assertions and broader bidirectional C interoperability.
The complete feature/platform checks and measured safe/unsafe comparison remain
explicit work items. None of these bounded successes alone completes the 188
unfinished official source units or the full roadmap.
