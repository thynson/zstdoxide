# Frozen direct Fast parser families

The default check uses only saved records and deterministic Python recipes:

    python3 scripts/import_fast_parser_families.py --check
    python3 -O scripts/import_fast_parser_families.py --check

Explicit regeneration requires a clean, already-built pinned reference checkout:

    python3 scripts/import_fast_parser_families.py \
      --upstream /path/to/zstd-v1.5.7 \
      --output /tmp/zstdoxide-fast-families-regeneration --sanitize

The output must be fresh and disjoint from the repository, collection and
reference checkout. Its evidence.json retains commands, environment, hashes,
compiler details and comparisons. Omit --sanitize for normal-only regeneration.
--collection supports isolated malformed-fixture tests. Native metadata is
observed anew; complete logical case records must match the frozen observations.
The frozen source driver and C helper live in scripts/oracles. Cargo never
builds or links the C helper.

Each normal-<id>.stdout fixture is JSON Lines: one metadata record and two
observations, padding 0x6a then 0xc3. recipes.json binds every source recipe,
packet and scalar before C execution. Source packets are reconstructed by the
independent driver rather than stored redundantly. The original nine-recipe
fast-parser smoke collection is unchanged.

The results below describe the original frozen capture. Subsequent portable
validation has its own evidence; repeated campaigns do not add distinct cases.

## Original capture results

All **142 recipes / 284 initialized-padding observations** passed the final
normal and scoped ASan/UBSan builds. The complete observations match across
builds. Every padding pair matches apart from its padding selector. The C
helper calls unchanged facebook/zstd v1.5.7 Fast, revision
f8745da6ff1ad1e7bab384bd1f9d742439278e99; no Rust Fast implementation or ordinary
frame/collector parity is claimed.

Final capture: `/tmp/zstdoxide-fast-direct-family-final/evidence.json`.
Final negative controls: `/tmp/zstdoxide-fast-direct-family-negative-final/summary.json`.
The native host is arm64 macOS, 64-bit little-endian. Other architectures have
not run this corpus. Compiler commands, version, source/archive identities and
effective environments are captured per command. Both compiler stderr files
are empty; there are no sanitizer errors or failed case assertions.

## Fixed corpus dimensions

| Family | Recipes |
| --- | ---: |
| Width / window-dispatch pairs | 30 |
| Selected conventional table logs | 12 |
| Short search tails | 28 |
| Acceleration and planted matches | 25 |
| Collision / paired lookup cases | 5 |
| Isolated conditional store | 2 |
| Preceding-match extension inputs | 8 |
| Available / disabled / duplicate repeat transitions | 12 |
| Ordinary / repeat / prefix-floor lower bounds | 8 |
| Long-length boundaries | 12 |
| **Total** | **142** |

The driver pins these counts and unique IDs before C execution. Requested
minMatch is 3..7, with effective hash widths 4..7. Hash logs are 6, 12, 16 and18;
windows include 10, 17, 18, 19 and20. Targets include 0..5 and131072. Larger
hash tables use complete-table canonical hashes plus reconstructible sorted
initial entries and changes; no table values are discarded from verification.

There are 23 explicit relational checks per build: requested-minMatch3/4
pairs, equal-bound window18/19 pairs, target0/1 equality and the isolated
step-four/five final-slot consequence. Individual case integrity and all source
bytes are independently replayed as well.

## Observed consequences

- **Lookup/store collision:** collision-adjacent emits no sequence; changing
  only the first block byte to avoid the shared bucket yields
  `(LL=1, ML=4, distance=65, offBase=68)`. The matching first-probe case yields
  `(0,4,64,67)`. These expose actual lookup/store consequences from unchanged
  Fast; they are not chronological per-instruction traces.
- **Conditional next-key write:** the 15-byte step-four/five cases have the
  same sequence `(1,4,65,68)`. Step four leaves bucket10 at C index70; step five
  leaves that bucket zero. Their other changed entries match. The short
  cleanup prevents later probes from obscuring this final-state witness.
- **Immediate repeats:** repeats-loop-available emits explicit distance96
  followed by three zero-literal offBase1 matches at distances32,96,32, all
  length32. The final parser cells are `[32,96,8]`; full wire repeat state is
  independently replayed and stored separately.
- **Disabled repeat restoration:** repeats-single-disabled emits one ordinary
  distance96 match and returns parser cells `[96,257,269]`, retaining the old
  disabled first offset as the new second. Separate no-new-offset and multiple-
  new-offset examples exist. The final automatically checked consequence list
  has no missing category.
- **Different lower-bound rules:** the ordinary window-bound-minus-one seed is
  rejected; equality yields distance960 and plus one yields959. Initial repeat
  distance1024 is used while1025 is disabled. The checker enforces the overall
  window and prefix floor but never substitutes the stronger block-end hash
  bound for repeat matches.
- **Long length fields:** the long-literal input emits LL65536, ML2048,
  distance65536 with the literal extension marker. Match length65538 has no
  extension marker;65539 does, matching the stored `mlBase=ML-3` boundary.
  The largest admitted block131072 is also captured.
- **Width and dispatch:** all requested3/4 comparisons agree after excluding
  only their selector/ID fields; all window18/19 equal-bound pairs agree after
  excluding only window/ID. Selected width5/6/7 and larger-table cases retain
  their complete observations rather than asserting they equal width4.

Preceding-match recipes distinguish explicit offsets from repeat intent and
capture changed LL/ML results. Their names count equal preceding input bytes;
they do not claim that the repeat branch extends backward by that many bytes.
That branch extends by at most one byte and may first discover the match at an
earlier sampled position. Post-match key presence is a final-state consequence,
not a reconstruction of every overwritten intermediate table write.

## Negative controls and preserved history

Final controls passed **114 commands**, with eight positive controls and106
expected rejections. Each normal/sanitized C executable rejected27 malformed
packets before producing a case record, including truncated/extra input,
malformed IDs, overflow/cap violations and invalid seeds. Each normal/optimized
Python run exercised28 record cases, including valid synthetic controls and
byte-replaying but semantically invalid window/prefix matches.

Preserved prior captures are qualified separately:

- Initial normal:132 recipes /264 observations, all passed; the missing named
  single-offset consequence remained explicitly recorded.
- Expanded normal and sanitized:142 recipes /284 observations each, passed.
- Final frozen-source normal and sanitized:142 recipes /284 observations each,
  passed after exact family count guards and a comment-only C cleanup.
- The first and final negative-control campaigns both passed; these are
  repeated validation, not additional distinct corpus coverage.

No failed compiler or sanitizer diagnostic is hidden. The initially missing
recipe consequence and the exact initial/expanded sources are preserved.

## Instrumentation and remaining scope

ASan/UBSan instrument the helper, unchanged zstd_fast.c and inline header code.
The remaining static archive support is uninstrumented, and LeakSanitizer is
disabled. Guard checks and explicit semantic bounds complement the reviewed
C setup; sanitizer success does not replace the pointer/ownership proof.

These are explicitly prepared direct no-dictionary states. nextToUpdate remains
unused index2, not an ordinary CCtx insertion-frontier observation. Dictionary
Fast, extDict/dictMatchState, collector behavior, ordinary-frame raw/RLE
publication, persistent multi-block direct state, rescaling, noncontiguous
streaming, LDM and workers are separate work. Existing independent Greedy has
not yet been run as a negative algorithm discriminator against these fixtures.
The fixture collection and observations do not establish full upstream test
migration, C-identical entropy bytes, compression ratio or performance parity.

See [the evidence record](../../../docs/evidence/2026-10-09-fast-parser-families.json) for retained logs and their precise scopes.

## Portable validation

The repository importer reproduced every observation in both normal and
sanitized builds: 289 successful commands and 568 C case executions. Every
complete case record matches the frozen capture; host-layout metadata is
observed separately. The independent wrapper review found no issue in offline
purity, source binding, output isolation or comparison scope.

Its ordinary/optimized Python offline campaign passed 54 commands with PATH
set to an unavailable directory: two valid controls and 52 expected rejections.
Controls include missing/extra/changed fixtures, a jointly forged observation
and manifest, substituted source hashes, detached host metadata, promoted
coverage/instrumentation claims, wrong counts, duplicate keys, escaping symlinks
and unsafe output-path overlaps. These supplement the original C packet and
semantic replay controls above. The generic audit now verifies 468 files in
twelve collections. No additional distinct parser recipe is claimed.
