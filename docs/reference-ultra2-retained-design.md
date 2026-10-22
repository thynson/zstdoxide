# Bounded retained Ultra2 design review

This review precedes implementation of Module 22. The reference is
facebook/zstd v1.5.7, revision
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`. Module 21's retained Ultra
implementation and Module 19's fresh Ultra2 implementation remain separate
capabilities. Shared policy-2 price arithmetic does not authorize either entry
point to accept a new lifecycle.

## Reference identities and eligibility

The reviewed `lib/compress/zstd_opt.c` SHA-256 is
`625936ee3fb02d789abb894c1fbc2918ee47582434094c963b630da13532c876`;
`lib/compress/zstd_compress.c` is
`10c315ae609d49b2fc431521aa95908086e69fd917991c5ed9a875af3d1208e8`.
The former's `ZSTD_compressBlock_btultra2` (lines 1517–1543) requires all five
bootstrap predicates: empty literal-length statistics, empty sequence store,
equal dictionary/low limits, current at the dictionary limit, and block size
greater than eight. `ZSTD_initStats_ultra` (1484–1508) separately requires the
unsigned dictionary-limit minus update-frontier difference to be at most one.
That helper precondition must not be silently added as a sixth dispatch gate.

`ZSTD_buildSeqStore` (zstd_compress.c, line 3264 onward) skips parser entry only
below seven bytes. Private first-block lengths seven, eight and nine therefore
test two different boundaries. Every proposed public first block exceeds eight.
The current position at any later block exceeds the unchanged dictionary limit,
so later blocks cannot bootstrap even if another individual predicate holds.

## Coordinates and retained ownership

Let `N` be complete source length, `B` first-block length and `S` the current
physical block start. Before seeding the normalized origin is zero. After
seeding it is `B`, the complete logical end is `B + N`, and a later block starts
at `B + S`. Native observations retain their additional constant bias of two;
normalization removes only that constant. The origin is never advanced again.
For public balanced schedules, `B <= 8192` and `B + N <= 40960`.

Source reads subtract the origin with checked arithmetic and validate the
requested width against the current consumed prefix. Stale cells below the
origin remain integers and cannot address input. Ring aliasing is not a
validity proof. Physical event/query/sequence offsets do not acquire the bias.
The strict 384/192 insertion catch-up rule compares logical positions on both
sides. Window admission and frame content size still use physical `N`.

| Owner or value | Lifetime and bootstrap behavior |
| --- | --- |
| Frequency tables | One retained allocation; learned statistics survive the discarded pass and later raw/RLE publication. |
| Path cells | One retained allocation; dormant cells remain observable. Phase cursors reset without zeroing the physical owner. |
| Parser candidates | One allocation with phase-local initialized lengths; discarded candidates cannot publish tokens. |
| Hash heads and tree links | Two retained allocations; first-pass cells become stale through coordinates, not pointer tricks or clearing. |
| Short-key heads | Existing optional owner and geometry; both insertion frontiers advance to the rebased origin. |
| Search candidates | Existing separate scratch owner; no alias with parser candidates. |
| Literal and sequence scratch | Two frame owners reserved before execution; discarded counts reset before the actual first pass. |
| Schedule, phase, repeats, coordinates | Copied checked values; shared safe/unsafe representation without artificial unsafe operations. |

These are the existing seven parser/search allocations plus two frame-token
allocations. Each performance-sensitive owner retains its existing independent
feature selector. No new feature combining unrelated owners is needed.

Initial repeats seed both first-block passes independently. Discarded repeats
never replace caller repeats. The actual pass produces pending repeats; only
compressed publication commits them and wire entropy. Raw/RLE publication
retains the search/price history while preserving committed wire state.
Failure after mutation poisons the whole coordinator until allocation-preserving
reset. Invalid scope, schedule or source identities reject before mutation.

## Independent evidence required before execution admission

The new model must reproduce the existing fresh Ultra2 lifecycle observations
and ordinary retained policy-2 observations under explicitly declared projections.
It must generate matches from source, never captured candidate lists. New phase
and block tags are separate from the two padding passes used by native observers.
Future-source variations must leave every earlier complete state unchanged.
Coherent forgeries must cover both sides of the rebase and later publication,
including table/sum changes, stale reads, owner swaps and repeat rollback.

The native observer must execute the actual upstream wrapper and preserve
parser/provider statements. Capture-plan bounds, dependency inventories,
sanitizers and independent audits precede fixtures and Rust parser admission.
Public strategy dispatch follows private state, wire, storage and platform gates.
The detailed work items are in `TODO.md`; ownership invariants are also recorded
in `data-structures.md`.

## Predeclared public predecessor changes

The current 1593-case corpus contains exactly two prospective resource-to-success
changes: index 1512 (`retained-resource-ultra2`) and index 1592
(`retained-ultra-resource-ultra2`). Both have source length 32768 and SHA-256
`b217b65e6f205f41b3fb8ef90cf7c44da93f630ca03965273485bbb21a5cccf5`.
Their requested window log is 16; the resolved window log is 15. Both request
hash/chain logs 12, search log 6, minimum match 3, target 4095 and strategy 9.
The request levels are 11 and 13 respectively. Every other predecessor response
must remain unchanged. This design does not yet change either classification.

The proposed additive public extent is 16385–32768 within one window, using
balanced three/four-block schedules. Fresh paths remain unchanged. Dictionaries,
prefixes, window expiry, numeric streaming, long-distance matching, workers and
complete upstream migration remain separate work.
