# Direct Greedy oracle protocol and allocation review plan

This source-only contract precedes helper implementation and execution. The
reference is unchanged facebook/zstd v1.5.7, revision
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`. The private no-dictionary entry points
are `ZSTD_compressBlock_greedy` and `ZSTD_compressBlock_greedy_row` in
`lib/compress/zstd_lazy.c`. Each execution constructs fresh match/sequence state.
It observes sequences and complete initial/final table contents, not an
ordinary CCtx trajectory or the chronology of intermediate stores.

The [Greedy design](reference-greedy-design.md) governs Rust admission. This
protocol deliberately permits either search branch for the same parameters to
isolate their semantics; public automatic selection remains based on window
log. No helper implementation, C output or successful new codec check is claimed
by this document. Actual allocation arithmetic and pointer setup must be reviewed
against this contract before the first helper execution.

## Packet version 1

One ordinary file supplies one bounded recipe. All integers are decoded as
unsigned little-endian values from bytes, never by casting a packed structure.

| Offset | Bytes | Field |
| --- | ---: | --- |
| 0 | 8 | Magic `ZGRDY01` followed by one zero byte |
| 8 | 64 | ID: 1..63 bytes from `[a-z0-9_-]`, followed only by zeros |
| 72 | 4 | branch: 0 for chain, 1 for row |
| 76 | 4 | prefix bytes P |
| 80 | 4 | block bytes B |
| 84 | 4 | window log W |
| 88 | 4 | hash log H |
| 92 | 4 | chain log C |
| 96 | 4 | search log S |
| 100 | 4 | requested minimum match M |
| 104 | 4 | target length T |
| 108 | 4 | retained prefix floor F, normalized source position |
| 112 | 4 | pending update position U, normalized source position |
| 116 | 12 | three positive initial repeat distances |
| 128 | 8 | explicit row salt |
| 136 | 4 | initial row salt entropy |
| 140 | 4 | ordered seed count K |
| 144 | 8 | two reserved u32 fields, both zero |
| 152 | 4*K | strictly increasing seed source positions |
| 152+4*K | P+B | initialized logical source bytes |
| end | 0 | exact EOF, with no trailing bytes or read error |

Bounds are P <= 2 MiB, 7 <= B <= 128 KiB, 10 <= W <= 21,
6 <= H <= 19, 6 <= C <= 18, 1 <= S <= 29, 3 <= M <= 7,
T <= 131072, F <= U <= P and K <= 524288. Additionally require B <= 2^W,
all shifts to fit size_t, N=P+B <= UINT32_MAX-2 and unchanged
`ZSTD_checkCParams({W,C,H,S,M,T,ZSTD_greedy})` to succeed. Chain execution
requires S <= 8 and zero salt/entropy; row execution uses the full explicit
salt and wrapping u32 entropy. Row mode consumes no chain owner. This initial
oracle's C bound is narrower than public row admission of unused chain logs;
later public interoperability must check that distinction.

The effective key width is clamp(M,4,6). Every seed p satisfies p < U,
p < P and effective_width <= P-p. Its selected logical key stays within the
prefix. Strictly increasing seeds and U after the last seed prevent self-links,
forward links and duplicate insertion into a chain before initial parsing.
Seeds older than F are allowed to exercise expiry. Arbitrary supplied table
cells are not accepted: complete initial maps are reconstructed from the
ordered seeds and returned in the observation. This replaces the preliminary
design's unspecified initial-map transport with a checked construction grammar.
Corrupt table-state tests belong to the Rust leaf tests, not unchecked C calls.

Maximum logical source is 2,228,224 bytes; maximum packet is
152 + 4*524288 + 2,228,224 = 4,325,528 bytes. Validate scalar ranges and checked
size arithmetic before any variable allocation, then reject an opened file
whose exact size differs. Require a regular file before opening and again on
the opened descriptor. Read the expected counts and exact EOF; a changing file,
truncated scalar/seed/source, invalid ID or reserved field is an error. The ID
must never become a file path. No stdin/network access is needed.

## Constructed state and initial tables

Start with zero-initialized, naturally aligned declared match/sequence records.
Keep dictionaries, optional parsers and unrelated tables null/zero. Set:

- Parameters to the validated explicit tuple, with Greedy strategy.
- `base = source - 2`, both window bases to base, nextSrc to source+N,
  dictLimit and lowLimit to F+2, and nextToUpdate to U+2.
- `loadedDictEnd=0`, `dictMatchState=NULL`, `lazySkipping=0` and empty hash cache.
- Chain: own `2^H` zeroed U32 heads and `2^C` zeroed U32 links. For each seed,
  use unchanged `ZSTD_hashPtr` at the effective width, write the old head to
  link slot `(p+2)&((1<<C)-1)`, then write p+2 to the head. This is setup, not
  expected output manufactured by the future Rust search.
- Row: own `2^H` zeroed U32 positions and `2^H` zeroed byte tags/heads. Set
  rowLog=clamp(S,4,6), rowHashLog=H-rowLog, explicit hashSalt/hashSaltEntropy.
  Require H>=rowLog and 1<=rowHashLog+8<=32. For each seed, use unchanged
  `ZSTD_hashPtrSalted`; row base is `(hash>>8)<<rowLog`, tag is hash&255.
  Within that row decrement head modulo row width, skipping zero, then assign
  head, tag and p+2. Cell zero of every position row remains zero. Record this
  explicit setup independently of any dictionary filler.
- Save full before-images only after seeding. Initialize sequence, literal and
  three code-array subranges and call unchanged `ZSTD_resetSeqStore`.

C zero is an invalid table index, whereas logical source position zero maps
to raw index two. Preserve the bias in serialized full maps, including chain
slot numbers. The Rust adapter may normalize positions, but must translate
cyclic slots explicitly. Table salt and dimensions are fixed for an invocation.
Initial lazy skipping is not a packet override because the parser resets it.

This direct contract does not perform the ordinary wrapper's cross-block
nextToUpdate adjustment. It will need a separately reviewed ordinary-frame
capture; do not infer that behavior from a hand-constructed U.

## Owners, alignment and cardinalities

Every helper owner retains its allocation base, aligned payload, requested
payload size and exact allocated extent. A guarded owner reserves
`max(payload_bytes,1) + 128 + 63` checked bytes. Align upward from base+64 to a
64-byte boundary; the adjustment is at most 63, leaving at least 64 bytes on
each side. Initialize the entire allocation, then its payload, and verify
**all** leading/trailing allocation padding and guard bytes before freeing.
Pointer rounding and bounds must be checked before forming the payload pointer.
C `uintptr_t` conversion and alignment assumptions are explicit host requirements;
record alignments and reject a host where an element needs alignment above 64
or not dividing 64. Do not apply these oracle requirements to scalar Rust Vecs.

All hash payloads are 64-byte aligned. A row base advances by at least 16 U32
cells, so every hash row meets `ZSTD_row_prefetch`'s 64-byte assertion. Byte tag
rows start at multiples of their 16/32/64-byte row width, also from an aligned
base. A plain malloc plus a fixed 64-byte guard is insufficient on hosts where
malloc only promises fundamental alignment.

Preflight checked payload and guarded costs for all simultaneously live owners
against a 64-MiB aggregate limit before constructing execution owners:

1. Packet seed positions (4*K bytes) and source (N bytes), both initialized.
2. Execution source arena and its identical before-image: each has 64 leading
   initialized bytes, N logical bytes and a 2048-byte initialized suffix.
3. Selected tables and full before-images: chain uses two copies of
   `4*(2^H + 2^C)`; row uses two copies of `5*2^H`. No inactive branch owner.
4. Sequence array Q*`sizeof(SeqDef)`, where Q=floor(B/3)+1 and pinned MINMATCH=3.
   Greedy matches consume at least four bytes, so this is conservative.
5. Literal bytes B+WILDCOPY_OVERLENGTH, with logical maxNbLit=B. Pin
   WILDCOPY_OVERLENGTH=32; its initialized permitted write slack precedes guards.
6. One 3*Q-byte owner for nonoverlapping initialized LL/ML/OF code subranges.
7. Independent initialized N-byte replay and Q expanded sequence observations.

Observe `sizeof` and alignment of U32, SeqDef, observation and state records.
Check allocation failures. On success, drop every owner and require exact zero
remaining helper allocation bytes. A failure exits nonzero, emits no complete
record and must not be treated as a valid partial capture. Fixed stack records
and libc/sanitizer overhead are outside the helper heap accounting, explicitly.

## Source pointer and short-input proof

The initialized source arena contains every source/base/current/candidate
pointer that the chosen no-dictionary parser forms from actual source indices:

- The 64-byte leading extent contains base=source-2, chain iend-8, row iend-16
  and literal-copy iend-32 even at P=0/B=7. Those expressions may be formed
  before discovering that no match loop runs. Padding is not logical history.
- Initial current is P+1 exactly when P==F, otherwise P. Main search requires
  current < N-8 for chain, current < N-16 for row. The first fresh searchable
  sizes are 10 and 18 respectively; with retained prefix they are 9 and 17.
  Inputs 0..6 belong to ordinary-frame bypass observations, not this protocol.
- Pending chain updates insert positions before the current probe; their
  physical four/eight-byte hashes fit before logical N. Row cached updates
  also hash position+8; a physical eight-byte load therefore ends at most
  current+16, covered by the stricter row search margin. Initial/refilled
  eight-entry cache fill stops at its inclusive limit. Prefix seed selected
  keys fit P; a physical eight-byte read may enter the initialized block.
- Seed indices are earlier than U. Subsequent insertions advance monotonically
  to the probe (row includes the probe), so candidates are earlier than the
  search. Chain traversal checks its moving floor and visit budget, and stops
  before dereferencing a cyclic slot at or below current-chainSize. That
  prevents reuse of an overwritten predecessor slot. Row collection is bounded
  by 64 and snapshots positions before writing the current row cell.
- Candidate four-byte improvement checks fit: the candidate is earlier than
  current, and best_length never exceeds N-current. Match counting ends at
  logical N. Source bytes before F are never treated as admitted history.
- Entry repeat validation saves/disables unavailable repeats. Its bitwise
  checks can still read with a disabled zero offset, which reads the current
  position rather than unallocated history. Positive distances are available
  at entry or arise from admitted earlier matches. Immediate repeats use an
  inclusive search limit, still leaving the required four bytes.
- Backward extension reads only after testing start>anchor and match>prefixLow.
  A decrement may land exactly on the floor. No earlier byte is read.
- On a miss, step=((current-anchor)>>8)+1 is at most 513 for admitted B.
  The final pointer after a failed search can exceed logical N by at most 513;
  the initialized 2048-byte suffix covers it and literal overread slack. Row
  prefetches address only validated allocated rows, with +16 U32/+32 byte
  subrow prefetches guarded by row width. No arbitrary source prefetch target
  or packet-supplied pointer exists.
- Literal copy follows upstream's explicit initialized read/write slack rules.
  Source and padding must remain byte-for-byte unchanged. Logical sequence
  replay consumes only N bytes; padding cannot make a match valid.

Run each direct recipe twice with physical padding 0x6a and 0xc3. All semantic
results must match after removing only the padding field. Guard bytes have
separate fixed patterns. An ASan pass does not replace this allocation review.
The unchanged upstream source uses `ZSTD_ALLOW_POINTER_OVERFLOW_ATTR` on several
relevant functions, and its compile-time no-dictionary specialization includes
unused null dictionary expressions. Retain and disclose upstream sanitizer
exclusions; do not claim a sanitizer pass proves ISO-C validity of every
upstream expression. New Rust implementations use checked integer positions.

## Observation and independent validation

A successful direct run writes exactly four newline-terminated JSON records:
metadata, padding-0x6a case, padding-0xc3 case, and complete. Metadata pins revision,
protocol, actual entry point, branch, constants, architecture, byte order,
owner sizes/alignment rules and `ordinary_frames=false`, `collector=false`,
`observation_kind="sequences-and-final-tables-not-chronological-trace"`.

Each case includes every packet scalar/seed, effective key and row dimensions,
source length/FNV, initial and final nextToUpdate, lazySkipping, salt/entropy,
all eight cache cells, raw C and independently replayed wire repeats, expanded
and raw sequences, stored literals, trailing length, long-length marker,
requested allocation total and guard/source/replay checks. Hash-cache cells
are C observations, not required Rust storage. Skip chronology and probe counts
must not be invented from final state.

Both selected owners use a full-map representation: entry count/element width,
sorted unique nonzero before cells, sorted unique changed `[index,old,new]`
triples and FNV over every canonical little-endian cell before/after. Row tags
are one-byte cells and include reserved heads; chain heads/links and row
positions are LE32 cells. Zero cells are included in the full fingerprint.
Initial positions/tags/heads are reconstructed independently from packet seeds
and the specified hash functions. Apply deltas only after checking old values,
index ranges and uniqueness. Check every reconstructed cell, row head, reserved
position, candidate range and salted key/tag relation; a link is a predecessor
index, not a hash-bucket cell. Do not apply head-bucket validation to links.

Independently expand the single optional long LL/ML marker, resolve wire
repeats using expanded LL, require positive distance <= window and available
prefix/current source, and replay overlap byte by byte. Consume stored literals
exactly, then append original-source trailing bytes and compare all N bytes.
Keep raw parser's unused third repeat distinct from semantic wire repeat three.
Full maps and raw sequences must agree between padding patterns and between
normal/instrumented builds. Packet/source SHA256 are external identities; FNV
alone does not bind capture files to the recipes.

The driver rejects duplicate JSON keys, unknown/missing fields, bool/float integer
substitutions, NaN, bad deltas, bad rows, bad long markers, invalid repeats,
trailing records and missing completion. Per-process stdout is capped at 128 MiB,
stderr at 1 MiB and runtime at 120 seconds, with timeout/kill/exit logs retained.
For compact output, a before cell is at most 17 bytes including its delimiter,
a delta at most 25, a seed at most 8 and a sequence record at most 256. Even
using these conservative widths for all 1048576 row cells, 524288 seeds,
43691 sequences, 262144 literal-hex bytes and 65536 other bytes, one case is
below 60 MiB. Both padding cases plus bounded metadata fit 128 MiB. Chain maps
have at most 786432 cells. Actual formatting must verify these per-record
bounds before first execution; no unbounded trace arrays are allowed.

## Fresh-context salt observation

Provide a separate `--fresh-salt` mode with no packet input. Create two distinct
fresh CCtx instances **sequentially**, observe zero pre-initialization salt and
entropy, and initialize each once for known source size 32768 with explicit
Greedy W15/C6/H6/S1/M4/T0, row enabled, workers/LDM disabled, no dictionary and
no checksum. Use the existing internal begin API after checked parameter setup;
inspect the applied tuple and enabled row mode rather than assuming them.
Return both before/after salt and entropy values as hexadecimal u64/u32, plus
rowHashLog, table counts and zero-initialization checks. Free each before the
next begins. Do not reset/reuse a context or compress data in this mode.

Opaque context allocation must use a checked custom allocator with a separate
8-MiB live cap and bounded owner tracking; keep allocation metadata separate
from the direct helper arena accounting. Each allocation must maintain normal
C alignment, detect overflow and retain the exact original free pointer.
Fill returned context payloads with 0x3b for the first context and 0xd1 for
the second, preserving separate guard patterns. This checks that zeroed state
and fresh salt come from upstream initialization, not helper calloc behavior.
Record the fill and complete releases. A source-derived `ZSTD_bitmix` calculation is an
independent cross-check; it must not replace the observed salt or pre-fill the
field being observed. This mode tests fresh-context salt only, not salt reuse,
dictionaries, frames or ordinary block selection.

## Capture gates and initial recipes

Before execution, review the implemented source against the packet, owner,
alignment and pointer proofs above. Compile the helper and every linked C
translation unit from pinned sources in normal and ASan/UBSan configurations;
link no prebuilt support archive. Preserve compiler identity, flags, dependency
closure, object/link hashes, upstream annotations/exclusions, return codes,
bounded logs and source hashes before/after. ASan/UBSan instrumentation of all
used units is distinct from absence of per-function upstream exclusions.

Reject output paths overlapping the repository, source checkout, protected
inputs or each other before creating any path or spawning a compiler. A capture
requires a fresh output directory. Offline replay imports no codec and launches
no process; exercise rejection controls with process creation disabled under
normal Python and `-O`.

First freeze tiny input hypotheses for both branches: fresh lengths 7/8/9/10
and 15/16/17/18, corresponding prefixed lengths 9/10/16/17, period-one/four
sources, M3/M4/M7 dispatch and unavailable repeats. Use explicit salt zero and
a fixed nonzero salt for initial packets; fresh-context observations remain
separately identified. Freeze exact recipes before running the helper. Wider
row widths, cyclic wrap/collisions, ties, floors, skipping, entropy wrap and
long-gap boundaries are the next separately decomposed capture item.
