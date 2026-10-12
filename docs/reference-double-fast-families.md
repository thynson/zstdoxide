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
