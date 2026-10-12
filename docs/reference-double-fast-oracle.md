# Initial direct DoubleFast oracle packet and safety plan

Prepared 2026-10-09; independently source-reviewed by the coordinator on 2026-10-12. No helper implementation, generated expected
sequence/table output, compilation, capture, tests or benchmarks accompany
this document. The authoritative scope is the current repository
`docs/reference-double-fast-design.md`; earlier private draft claims that every
ordinary candidate needs to be strictly above the floor are superseded.
Reference: facebook/zstd v1.5.7, revision
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`.

The above is the initial design checkpoint. Implementation, pre-execution
allocation review and the first fifteen captured cases were subsequently
completed on 2026-10-12; see [adapter review](reference-double-fast-adapter-review.md)
and [validation](reference-double-fast-validation.md). Wider families remain open.

Call only unchanged `ZSTD_compressBlock_doubleFast` from
`lib/compress/zstd_double_fast.c:568-585`, entering its noDict implementation
at :105-324. This initial helper does not construct ordinary frames, use the
public sequence collector, load dictionaries, or assert a realistic whole-CCtx
trajectory for its manually seeded state. Every invocation owns fresh state.

## Binary input: one bounded recipe per process

Proposed protocol name `ZDFST01`, schema 1. All integers are unsigned little
endian, decoded from bytes without structure casts or host-endian assumptions.
The input file contains exactly:

| Offset | Size | Field |
| --- | ---: | --- |
| 0 | 8 | ASCII bytes `ZDFST01` followed by one zero byte |
| 8 | 64 | ID: 1..63 bytes in `[a-z0-9_-]`, followed only by zeros |
| 72 | 4 | prefix_bytes P |
| 76 | 4 | block_bytes B |
| 80 | 4 | window_log W |
| 84 | 4 | long_hash_log H, mapped to cParams.hashLog |
| 88 | 4 | short_hash_log C, mapped to cParams.chainLog |
| 92 | 4 | search_log S |
| 96 | 4 | requested_min_match M |
| 100 | 4 | target_length T |
| 104 | 4 | retained_prefix_floor F, normalized source index |
| 108 | 4 | initial repeat 1 |
| 112 | 4 | initial repeat 2 |
| 116 | 4 | initial repeat 3 |
| 120 | 4 | long_seed_count NL |
| 124 | 4 | short_seed_count NS |
| 128 | 4 | reserved, exactly zero |
| 132 | 4*NL | ordered long seed source positions |
| 132+4*NL | 4*NS | ordered short seed source positions |
| 132+4*(NL+NS) | P+B | exact initialized logical source bytes |
| end | 0 | mandatory exact EOF, with no trailing bytes or read error |

Admit P in 0..=2 MiB, B in 7..=128 KiB, W in 10..=21, H and C in
6..=18, M in 3..=7, S in 1..=29 and T in 0..=131072. The S cap is a
portable initial envelope subset, not a new public parameter validity rule;
all seven cParams must also pass unchanged `ZSTD_checkCParams` before execution.
DoubleFast does not consume S or T during this noDict search. The short effective
key width K=max(M,4), and the long width is exactly 8.

Require B <= 2^W, 0 <= F <= P, NL/NS <=1024, and three positive u32 repeat
cells. Large initial repeats are permitted because the unchanged entry logic
saves and disables unavailable ones before candidate use. Maximum logical
source N=P+B is 2,228,224 bytes and must also fit `UINT32_MAX-2` and size_t.
The maximum complete packet is 132+8192+2,228,224 = 2,236,548 bytes.

Validate all scalar domains, shifts, counts and checked packet-size arithmetic
before allocating source or reading variable-length arrays. Reject truncation
at every field/seed/source boundary, invalid IDs, nonzero reserved fields and
extra EOF bytes. Open only an ordinary caller-provided file; no stdin polling,
network access or path construction from the recipe ID. Retain malformed-input
stderr/exit records, but never treat a partial output as a valid capture.

Long seeds satisfy p<P and p+8<=P. Short seeds satisfy p<P and p+K<=P.
The additions must be checked or rewritten as K<=P-p after proving p<P.
Each list preserves explicit insertion order. Repeated positions and colliding
buckets are permitted, with the last listed write winning; do not silently sort
the seeds. Seeds may be older than F so expired-candidate rejection can be
observed. They must never name future/current-block positions. These are exact
constructed table contents, not a claim that C's dictionary filler generated
them. Short hash widths5..7 physically read eight bytes in C; a seed near the
prefix end may therefore read into the initialized current block, but its
selected logical K-byte key must remain wholly in the prefix.

## Fresh state and distinct floor rules

Use all-zero fresh `ZSTD_MatchState_t` and `SeqStore_t`, then assign only the
explicit fields below. Their complete sizes/alignments and pinned constants are
observed in metadata. Keep `loadedDictEnd=0`, `dictMatchState=NULL` and unrelated
optional owners/tables null. Set:

- cParams = {W,C,H,S,M,T,ZSTD_dfast}; validate unchanged C parameters.
- source points to the initialized logical bytes; base=source-2; require pinned
  `ZSTD_WINDOW_START_INDEX==2` and `HASH_READ_SIZE==8`.
- `window.base=window.dictBase=base` and `window.nextSrc=source+N`.
- `window.dictLimit=window.lowLimit=F+2`.
- `hashTable=long_table`, `chainTable=short_table`, `nextToUpdate=2`.
- zero all `2^H` long and `2^C` short U32 cells, then apply ordered seeds with
  unchanged `ZSTD_hashPtr` at widths8 and K and store p+2.
- build guarded sequence/literal/code-array owners, set their pointers and
  logical capacities, then call unchanged `ZSTD_resetSeqStore`.

The direct parser does not use nextToUpdate or the independent lowLimit to
select prefix bounds. Those fixed assignments are explicit constructed-state
metadata, not evidence of an ordinary CCtx insertion frontier. In this first
protocol F drives dictLimit, with lowLimit set equal to it; there is no hidden
second externally controlled floor. A future independent lowLimit experiment
requires a new documented field/version, not a reinterpretation of F.

Let E=P+B and L=max(F, E-2^W), interpreting the subtraction as saturating.
The B<=window requirement proves L<=P. Initial ip is P+1 only when P==L,
otherwise P. Repeat availability is calculated separately at that initial
current position, using max(F,current-window), as C does.

**Current long and current short candidates accept an index equal to L.**
`ZSTD_selectAddr` at `zstd_compress_internal.h:627-644` returns the candidate
for index>=lowLimit, including the x86 assembly branch. **The later-long probe
uses an explicit strict index>L check** (`zstd_double_fast.c:275`). Backward
extension tests `match>floor` before decrement, and may finish exactly at L.
Do not use the misleading nearby current-candidate comment as the comparison.
An empty table cell 0 maps to invalid, while normalized logical position0 maps
to raw C index2 and can be a valid current candidate at the floor.

For seven/eight-byte blocks, the parser performs cleanup without hash access.
A nine-byte block can search with prior history keeping ip=P; a fresh block
with P=L skips one byte and needs ten bytes to enter the loop. A caller must not
reuse Fast's different paired-search stop predicate. Lengths0..6 are excluded
from the direct packet and belong to the later ordinary-frame bypass capture.

## Owned allocations and arena proof

Reuse the reviewed Guarded ownership pattern, but inventory **two** table
allocations and **two** before-image allocations. Every helper allocation uses
checked size addition/multiplication, tests malloc failure and has distinct
64-byte leading/trailing canaries. All payload bytes are initialized before C
can access them. The requested allocation sum, including guards, must be
computed and limited to 64 MiB before constructing the execution owners.
That limit is for this test helper, not the Rust codec's API.

Allocations are:

1. The bounded input source N, owned by recipe parsing.
2. An execution source arena and an identical before-image. Each arena payload
   has 64 initialized leading bytes, N logical source bytes and a fixed
   `MAX_BLOCK_BYTES+512` = 131,584-byte initialized suffix.
3. Long and short U32 tables and their complete before-images. Each table uses
   checked `2^log * sizeof(U32)`. At max logs, all four payloads total4 MiB.
4. Sequence array Q*`sizeof(SeqDef)`, with Q=floor(B/MINMATCH)+1 and pinned
   MINMATCH==3. Each emitted match consumes at least four source bytes, so this
   conservative capacity is sufficient even with zero-length literals.
5. Literal owner with logical `maxNbLit=B` and physical B+WILDCOPY_OVERLENGTH
   initialized bytes. The trailing canary follows this permitted write slack,
   not the logical literal count.
6. Three code arrays in one initialized 3*Q-byte owner, with distinct LL/ML/OF
   subranges. They are valid even though this direct parser does not encode
   entropy codes.
7. An N-byte initialized independent replay buffer and a Q-element observation
   owner (expanded LL/ML, raw offBase and resolved distance).

A malloc base has required fundamental alignment. Before typed casts, require
64 modulo each of `_Alignof(U32)`, `_Alignof(SeqDef)` and
`_Alignof(Observation)` to equal zero. The fixed guard offset then preserves
alignment. Do not infer alignment solely from the apparent address. State/store
are ordinary properly aligned declared objects, zero-initialized explicitly.
Metadata records their `sizeof` and `_Alignof`; they own no extra C buffers.

### Pointer formation inside the unchanged parser

All source/base/current/candidate/prefetch pointers remain within the *same*
initialized arena, not merely an adjacent allocation:

- base=source-2 retains62 physical bytes before base. For B>=7,
  `iend-8` lies at least source-1, and literal wildcopy's `iend-32` lies at least
  source-25, still inside the64-byte leading extent even at P=0.
- `prefixLowest` lies in source[F..=P]. Initial ip lies in P..=P+1.
  Entry/outer cleanup can form ip+1 and ip+256 even without any subsequent
  load. The fixed suffix covers these expressions.
- Inner ip1 values used for loads are <=iend-8. C's long loads and hash8 reads
  therefore end no later than iend. Short physical hash reads also fit eight
  initialized bytes. Prefix candidates came from validated prefix seeds or
  earlier parser insertions, and their selected keys fit the logical source.
- Each unsuccessful probe advances at least one byte. Conservatively there
  are no more than B step increments in a search, so step<=B+1. The final
  `ip1 += step` pointer, formed before loop termination, is no farther than
  iend+MAX_BLOCK_BYTES+1. The chosen suffix131584 covers it. `nextStep` starts
  at ip+256; later increments are gated by old nextStep<=ip1<=iend-8, so a
  subsequent +256 cannot exceed iend+248. The explicit +64/+128 prefetches and
  architecture-specific `ip+256` are covered as well.
- Zero table cells form base+0 inside the arena, but current-candidate selection
  redirects them to C's separate initialized ten-byte dummy array. Its eight-
  and four-byte reads fit. Candidate identity is checked along with equality.
- Bitwise repeat conditions evaluate their reads even when a local repeat was
  disabled to zero. The zero case reads ip+1 or ip itself, within the search
  tail. Positive repeats are admitted at entry or arise from proven earlier
  matches; they never point before available initialized prefix. The third
  input repeat is metadata only for this parser and remains untouched.
- Backward reads use the pre-decrement anchor/floor checks. Reaching the floor
  is valid, moving below it is not. Complementary writes use long(curr+2),
  long(ip-2), short(curr+2), short(ip-1), after the ip<=iend-8 guard. Keep that
  per-table collision order. The deferred long(ip1) write is guarded by step<4,
  proving it is strictly before the new match end; no future-position seed is
  manufactured. Immediate repeat-loop starts update short then long tables.
- `ZSTD_count` reads bounded source ranges; match counting never crosses logical
  iend. `ZSTD_storeSeq` may perform its documented initial literal copy/wildcopy
  in initialized physical source/destination slack. Guard positions must not
  mistake that permission for a logical source extension.

The final C code still requires an independent reviewer to verify this proof
against actual allocation arithmetic and every assigned field before execution.
An ASan pass cannot replace pointer-formation reasoning.

## Output: strict JSON Lines and independent semantic validation

Successful stdout contains exactly four newline-terminated records:

1. `record="metadata"`: schema1, exact pinned revision/entry point, strategy
   DoubleFast, protocol magic, constants/caps, guard/arena sizes, actual pointer
   bits/endian/U32/SeqDef/match-state sizes and alignments, and explicit
   `ordinary_frames=false`, `collector=false`, and
   `observation_kind="sequences-and-final-tables-not-chronological-trace"`.
2. `record="case"`, padding106 (0x6a), complete observation below.
3. `record="case"`, padding195 (0xc3), complete observation below.
4. `record="complete"`, recipe ID, recipes1, observations2, success true.

No extra records, repeated JSON keys, floats, NaNs, unknown fields, unexpected
nulls or bool-as-integer substitutions are accepted. The driver binds metadata
to the collection manifest and validates every integer's exact type/range.
Require bounded stdout files <=64 MiB; this covers the admitted sequence and
table-delta counts with conservative decimal/hex field sizes. Initial recipes
are much smaller. A failed process/parse cannot produce accepted evidence even
if it previously printed valid-looking case records.

Each case contains:

- exact ID, padding, all input scalar fields, effective widths, initial repeat
  triple, both ordered seed arrays, source byte count and FNV1a64;
- fixed state assignments (raw initial dictLimit/lowLimit/nextToUpdate, null
  dictionary state, loadedDictEnd0), calculated current/ordinary floor values,
  and all physical sizes/logical output capacities;
- sequence count, stored-literal byte count/hex, returned trailing literal count,
  raw longLengthType/longLengthPos, and exact sequence rows. A row records the
  raw u16 LL/mlBase and u32 offBase plus independently expanded LL/ML, resolved
  distance and an intent tag mechanically consistent with offBase;
- separate `parser_repeats` and `wire_repeats` triples;
- `long_table` and `short_table` objects, each with layout/entry count,
  sorted unique `[bucket,raw_index]` nonzero before cells, sorted unique
  `[bucket,old_raw,new_raw]` changes and full before/after LE32 FNV1a64 hashes;
- successful guard/source-and-slack-unchanged checks and independent replay flag.

Driver-side packet/source SHA256 and exact byte lengths are external capture
metadata; they bind what C actually received to the independently rebuilt
recipe. Do not substitute an in-helper FNV for artifact identity. The C helper
and Python validator independently:

- expand a single permitted long LL or ML by65536 at the validated position;
  ML also adds MINMATCH. Use the **expanded** LL for zero-literal repeat rules;
- resolve offBase and update all three semantic wire repeats without calling
  `ZSTD_updateRep` or collector APIs; require positive distances;
- append only available stored literals, then require distance<=cursor,
  distance<=2^W and cursor-distance>=F, with enough block space for ML;
- replay overlapping matches byte by byte in initialized output, consume every
  stored literal exactly once, append returned trailing bytes from the original
  source, and compare the full reconstructed prefix+block to the packet source;
- verify the two parser-used repeat cells against semantic replay and preserve
  the original untouched third C parser cell separately;
- recreate each zero table, apply ordered seeds through an independent exact
  selected-byte hash, compare every before nonzero cell, apply deltas after
  proving their old values, and recompute hashes over every canonical LE32 cell;
- require every final nonzero raw index>=2, its selected long8/shortK key wholly
  inside logical source, and its independently computed bucket equal the slot.

For replay, do **not** impose L on every match: repeat eligibility can reach
back before the stronger block-end floor while remaining within window/F.
No result-only validator can identify every internal branch from offBase alone;
current>=L versus later>L witnesses need isolated recipes plus inspected source,
with carefully stated observed consequences. Canonical table deltas do not
reveal the order of all intermediate stores.

Compare the two padding records completely after removing only their `padding`
field. Layouts, hashes, sequences, literals, repeats, guards and semantic flags
must otherwise agree; merely recording both patterns is insufficient.

## Initial hand-readable recipe hypotheses, with no expected outputs

All listed cases use W10/H6/C6/S1/M4/T0/F0 and repeats[1,4,8] unless stated.
Seeds are empty unless stated. Byte strings are literal ASCII and repetition
counts are exact. These recipes are proposed inputs, not observed branch/output
claims. Do not synthesize their expected sequences with the future Rust parser.

| ID | Prefix and block | Overrides / purpose |
| --- | --- | --- |
| fresh-seven | P0; block `abcdefg` | Short cleanup |
| fresh-eight | P0; block `abcdefgh` | Eight-byte boundary |
| fresh-nine | P0; block `abcdefghi` | Fresh start skips one byte |
| fresh-ten | P0; block `abcdefghij` | First fresh searchable length |
| prefix-nine | prefix `0123456789ABCDEF`; block `012345678` | Long seeds[0], short seeds[0]; repeats[257,263,269] |
| prefix-ten | same prefix; block `0123456789` | Same seeds/repeats; contrast prefix search boundary |
| repeat-one | P0; block `A` repeated64 | Overlap and initial repeat use |
| repeat-four | P0; block `abcd` repeated16 | Four-byte period |
| current-long-floor | prefix `0123456789ABCDEF`; block same16 bytes | Long seeds[0] only; repeats[257,263,269]; current floor equality |
| current-short-floor | prefix `abcdWXYZ01234567`; block `abcdQRSTuvwx1234` | Short seeds[0] only; repeats[257,263,269]; short floor equality |
| later-floor | prefix `ABCDEFGH01234567xABCpqrs`; block `xABCDEFGH0123456` | Long seeds[0], short seeds[16], repeats[257,263,269]; later candidate exactly at floor |
| later-above-floor | prefix `_ABCDEFGH01234567xABCpqrs`; same block | Long seeds[1], short seeds[17], same repeats; later candidate above floor |
| extend-to-floor | prefix `abcdefghijklmnop`; block prefix repeated twice | Long seeds[4], same unavailable repeats; potential back-extension to floor |
| dispatch-three | Same input as repeat-four | M3; four-byte dispatch comparison |
| ignored-controls | Same input as repeat-four | S29/T131072; noDict DoubleFast control invariance |

Initial envelope contains15 independently named recipes, each with two padding
runs. A later recipe implementation must verify these declared byte lengths and
seed domains before capture. Any accidental bucket collision that prevents an
intended branch remains an observed limitation of that recipe; preserve its
first diagnostics before adding a new isolated witness. Do not change semantic
expectations merely to obtain a branch claim. Wider short keys, larger logs,
step3/4 and256 thresholds, unequal/tied long lookahead, multiple immediate
repeats and long-length markers are a separately decomposed family checkpoint.

## Reuse, changes, publication and future execution gates

Reusable designs from `scripts/oracles/fast_direct_families.{c,py}` and
`scripts/import_fast_parser_families.py` are: checked Guarded allocations,
explicit LE packet I/O, independent full-repeat replay, long-length handling,
paired initialized padding, canonical table maps/hashes, deterministic recipe
pinning, source catalogs, bounded process logs and compiler-free fixture checks.
Reuse cannot mean mutating frozen Fast assets or treating Fast expected values
as DoubleFast observations.

Required changes are: new magic/schema and named collection; two independent
seed lists/table owners/before maps/deltas; chainLog as the short dimension;
width8 hashing including private logs6..9 (LE64 multiply by
0xcf1bbcdcb7a56463, wrapped modulo2^64 then high H bits); no Fast target-derived
step or raw-literal policy; W21 cap; the correct distinct candidate floors;
DoubleFast pointer-slack proof and exact7/8/9/10 entry behavior. Importer should
verify the frozen driver identity before executing imported helper code, and
bind a fixed independently frozen fixture catalog in addition to its manifest.

Future normal and scoped ASan/UBSan builds should explicitly compile unchanged
`zstd_double_fast.c` in both modes with the new setup helper, then link the
pinned archive for support functions. This avoids attributing an unverified
archived parser object to the inspected source. Instrumented scope is helper,
DoubleFast and inline headers, not every support/archive routine; record
LeakSanitizer settings rather than implying whole-codec sanitizer coverage.
Record complete argv/effective environment/compiler version and binary identity,
source/header/archive closure hashes before and after, return codes, bounded
stdout/stderr hashes, including TimeoutExpired/OSError and partial logs.

Before creating output/report paths, resolve them and reject either direction
of overlap with the repository, reference checkout, helper/fixture collection,
each other, and protected manifest inputs. Require a fresh output directory.
Offline `--check` performs no compilation/subprocess, rejects live-only options,
checks exact file catalog/identities/schema/source/semantic invariants and
reconstructs recipes independently. Test malformed packets and forged records
under normal Python and `-O`, including duplicate keys, bool scalars, malformed
long markers, invalid table seeds/deltas/hashes, wrong metadata/padding pair,
source/floor violations, EOF mismatch and report/output aliasing. Implementation and capture follow Module 12 completion. The coordinator
will verify the implemented arena and protocol against this contract before
executing the helper.

## Coordinator review

The coordinator separately inspected the pinned noDict loop, minMatch dispatch,
ZSTD_selectAddr comparison, hash8 implementation and the existing Fast oracle
setup on 2026-10-12. This reviewed protocol preserves the two independent table
owners, selected versus physically read key widths, normalization of source
position zero, and separate current/later candidate admission. The conservative
suffix covers formed search/prefetch pointers as well as dereferenced bytes.
The tiny recipes remain hypotheses; final table maps do not prove intermediate
write chronology. No generated C or Rust observations are claimed here.

Reviewing this design does not substitute for checking the helper's actual
allocation arithmetic, pointer assignments, output bounds and cleanup paths
before its first execution. That check is part of the next work item.
