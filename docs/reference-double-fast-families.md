# Wider constructed DoubleFast recipe plan

## Scope and ordering

This plan follows the frozen fifteen tiny observations and 512 direct small-log
hash vectors. Fix all recipe inputs and their identities before capturing new
parser outputs. Reuse the exact reviewed ZDFST01 C helper and its bounded
arenas; no parser instrumentation or changed upstream function is required.
Every case remains a fresh, manually seeded no-dictionary state. Final table
maps cannot prove all intermediate store chronology or ordinary CCtx state.

The proposed envelope has 168 unique recipes in fourteen families. All cases
must pass the existing packet/seed/window/allocation constraints. Capture later
under both normal and scoped sanitized builds, with both physical padding
patterns and full independent replay. Recipe descriptions below are hypotheses,
not generated expected C sequences. Keep first diagnostics when a collision or
different search path prevents an intended witness, then add a separately named
refinement if needed; never quietly replace the frozen source-only hypotheses.

| Family | Count | Fixed input choices and intended boundary |
| --- | ---: | --- |
| width-dispatch | 30 | Three deterministic period/split/asymmetric inputs, minMatch3..7, windows18/21; distinguish selected short widths and unchanged window bounds. |
| table-shape | 20 | (long,short) logs (6,9), (9,6), (10,18), (18,10), (18,18), each at widths4..7; independent dimensions. |
| tail | 34 | Fresh and seeded-prefix variants at lengths7..17,23,24,25,31,32,33. |
| sampling-tail | 25 | Unique-four-byte sources of lengths253..260,509..516,765..773; retain complete sampled tables around growth thresholds. |
| sampling-plant | 10 | A distinct high-byte prefix key planted at block positions255,256,257,511,512,513,767,768,770,771 in otherwise unique-four-byte data. |
| conditional-store | 2 | A four-byte prefix match at block position512 or770, with minimal cleanup tail and isolated deferred long bucket; hypothesize step3/4. |
| lookahead-length | 3 | A twelve-byte short candidate and later long lengths11,12,13; isolate shorter/tied/strictly-longer choice. |
| current-long-priority | 2 | Same input has current long length16 and current short length20; compare seeded/empty long table. |
| collision | 2 | Bounded search for distinct adjacent eight-byte keys sharing a log6 bucket; paired noncolliding first-byte control retains the later prefix key. |
| seed-order | 3 | Repeated/colliding prefix keys with distinct continuations and explicit ordered seeds; last initial write wins. |
| repeat-transition | 12 | Five multipart sources with available/disabled repeats, plus a single-new-offset cleanup and duplicate-repeat case. |
| floor | 11 | Ordinary candidate at block-end lower bound minus1/equal/plus1 in both table roles; repeat distances1024/1025; three explicit prefix floors. |
| long-marker | 12 | Eleven repeated-byte blocks near raw match-base65536 and the maximum block; a unique 65536-byte literal candidate followed by a high-byte prefix match. |
| envelope | 2 | Maximum2MiB prefix with block7, and maximum prefix/block/table dimensions; distinguish helper bounds from public API policy. |

## Deterministic input construction

Use a fixed unsigned32 LCG for noise, with multiplier 1664525 and increment
1013904223, mixing the high byte with bits13..20. A base16 order4 de Bruijn
sequence supplies 65536 cyclic positions with distinct four-byte words. Check
that uniqueness independently, including the cyclic boundary. These sources
describe inputs only; neither generator performs DoubleFast parsing.

Planted keys use bytes above the de Bruijn alphabet to avoid accidental
four-byte equality in unmodified background data. Short-only match witnesses
explicitly differ at byte4. The conditional-store recipes use their declared
block-relative positions, and end at position+step+8 so the later long key is
complete. Before capture, verify its bucket is distinct from every potential
earlier block insertion and the complementary position+2, using a bounded
choice of background slices if necessary. This isolates a possible final-store
consequence without pretending an unobserved branch was taken.

Long-lookahead recipes place the later prefix candidate strictly above floor0,
prevent backward extension by changing its preceding byte, and terminate both
candidate continuations explicitly. Their current/later long buckets must be
different. The collision pair deliberately reverses that last condition while
retaining different actual eight-byte values and the same later source key.
Collision enumeration is finite and records the selected bytes in source hashes.

The long-literal candidate uses a high-byte repeated prefix and only a complete
prior seed, then de Bruijn data and a 128-byte matching tail. Initial repeats are
unavailable. Whether its first observed LL needs the long marker is measured
later. Do not infer a marker solely from input length. Likewise, a final table
map can demonstrate that a specified bucket changed, but cannot by itself
identify every transient write or prove an intermediate branch trace.

## Pre-capture checks and publication

Record exact family counts, every scalar, ordered seed lists, source/packet
lengths and SHA-256 identities. Reconstruct these independently in normal and
optimized Python, rejecting invalid domains, unknown/duplicate case names,
altered manifests, incomplete/extra entries and changed generator identities.
Preserve the tiny helper, driver and fixtures byte-for-byte. The recipe
checkpoint contains no new C output; later capture is a separate work item.

## Completed source-only checkpoint

The generator and manifest were frozen on 2026-10-12 with all 168 recipes and
the exact fourteen family counts above. The source bytes total 5,242,885; the
largest complete packet is 2,228,372 bytes. The compiler-free checker verifies
the generator and reused helper identities before importing recipe code,
reconstructs every source/packet hash, and independently decodes the packet
fields, selected-key domains, seed order and exact EOF extent.

Normal and optimized Python both pass the complete envelope and 52 rejection
controls covering malformed packets, invalid typed recipe parameters and
changed/incomplete/duplicate/unknown manifest entries. The unchanged tiny
helper, direct validator and observations retain their recorded identities.
Both project check scripts now run this input check. No C subprocess, Rust
parser, performance measurement or extra Clippy/no_std run is attributed to
this source-only checkpoint. See [evidence](evidence/2026-10-12-double-fast-recipes.json)
and the [source/control archive](evidence/2026-10-12-double-fast-recipes.tar.gz).

## Completed capture, observed limits and refinement

The original 168 inputs subsequently pass 343 capture commands in normal and
scoped ASan/UBSan builds. Both builds explicitly compile unchanged DoubleFast
with the already reviewed helper. All 672 padding-case executions agree across
builds and padding patterns, with complete independent sequence/literal/repeat
replay and both entire table maps. Source/header/helper/driver/packet identities
remain unchanged before and after capture. The remaining support archive is
uninstrumented, and LeakSanitizer is disabled explicitly.

The original long-literal candidate produces **no sequence**. Its largest final
sampled table position is block-relative 65298, before the matching tail begins
at 65536. Both long and short prefix seeds remain present. This result is
consistent with the growing-step search and its lookahead stop condition; it
does not establish a chronological probe trace. The failed marker hypothesis
and complete original output remain frozen.

A separately named `long-literal-extended-tail` input was fixed before its own
execution, extending the matching tail from 128 to 1024 bytes. Its source and
packet identities are recorded in `refinement-long-literal.json`, without
changing the original manifest or generator. Two further commands reuse the
exact recorded compiled normal/sanitized helpers after checking their binary
and source identities. Four padding-case executions agree and observe LL65554
as raw LL18 plus the long-literal marker at sequence0. The sequences are
(LL65554, ML64, distance65618) and (LL0, ML942, distance2), with no trailing bytes.

Other isolated measured consequences include:

| Boundary | Observation |
| --- | --- |
| Step3 deferred long store | Bucket215372 becomes raw index549. |
| Step4 deferred long store | Isolated bucket69048 stays empty; hypothetical candidate raw index808 is not stored. |
| Short length12 versus later long11/12 | The current short candidate remains selected in both cases. |
| Short length12 versus later long13 | The later candidate wins with LL1 and distance33. |
| Current long16 versus short20 | The seeded long candidate wins; removing it selects short20. |
| Adjacent long-bucket collision | Colliding input retains short4; the paired control selects later long8. |
| Immediate repeats | One explicit sequence followed by three zero-LL repeat sequences swaps the first two wire repeats correctly. |
| Ordinary floor540 | Both current table roles reject539 and accept540/541. |
| Repeat window | Distance1024 yields a repeat match; distance1025 remains unavailable. |
| Raw match-base boundary | Block65540 gives raw ML base65535 without a marker; block65541 gives base0 with the long-match marker. |

The combined collection has 169 cases and 338 padding observations, representing
676 C executions across both builds. The offline checker validates 22 whole-record
equivalence pairs and 20 sequence witnesses, alongside every generic replay/table
check. Normal and optimized Python both pass all observations plus 2 positive
and 32 rejection controls. Rejections include malformed long-marker records,
changed table cells, false witness/equivalence claims and a repaired but forged
catalog. The exact fixture catalog is independently pinned, and recipe-code
identities are verified before import. Both project check scripts include the checker; the registry
passes in both Python modes with 715 files in 19 collections. Whitespace checks
pass. This oracle-only checkpoint runs no new Rust parser, performance or
foreign-platform test, and changes no production Rust code.

See [combined evidence](evidence/2026-10-12-double-fast-families.json) and the
[source, command, refinement and control archive](evidence/2026-10-12-double-fast-families.tar.gz).
The next implementation items consume these observations; ordinary frame
publication trajectories remain a separate capture.
