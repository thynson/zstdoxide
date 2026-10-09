# Independent reference Fast sequence and state oracle

Module 12 oracle design, 2026-10-09. These are planned captures and checks,
not completed C execution or fixture results. This complements
[the Fast execution design](reference-fast-design.md).

Reference: facebook/zstd v1.5.7, revision
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`, inspected at
`/tmp/zstdoxize-upstream-1.5.7`. The reference checkout's tracked status was clean.
All source paths and line numbers below are relative to that pinned checkout.

Read-only source SHA-256 observations:

- `lib/compress/zstd_fast.c`: `8037320f321836652e73ab6cbf7b8f1f4c97cef38835ac6b4aaf0af716aa398d`
- `lib/compress/zstd_compress.c`: `10c315ae609d49b2fc431521aa95908086e69fd917991c5ed9a875af3d1208e8`
- `lib/compress/zstd_compress_internal.h`: `ab7754413cef565da559bda7eb738bc2b3708c48877cb9ec37aff767d32c57d1`
- `lib/zstd.h`: `9b4bc8245565c98ccfc61c07749928b57e7c0f6fddb0530c4f6aa1971893d88b`

## Recommendation

Freeze three explicitly distinct observations before implementing Rust Fast:

1. **Direct parser:** call unchanged `ZSTD_compressBlock_fast` with bounded,
   initialized no-dictionary match state and sequence storage. Record logical
   sequences, explicit offset intent, trailing literals, parser repeat cells,
   and complete before/after hash-table values.
2. **Collector corroboration:** call unchanged `ZSTD_generateSequences` on
   separately initialized contexts. Compare its logical sequence observations
   to a direct-parser campaign with the collector's own publication policy.
3. **Ordinary frame publication:** call unchanged frame compression one block
   at a time, inspect its actual block headers and state before the next call,
   and independently decode the complete frame. This determines whether raw,
   RLE, or compressed selection commits candidate repeat/entropy state.

The first two are candidate-parser observations. They must not be substituted
for the third. None requires modifying upstream parser code, binding production
Rust to C, or requiring identical entropy-encoded frame bytes from Rust.

## Source facts which constrain the oracle

| Source | Contract or qualification |
| --- | --- |
| `lib/zstd.h:1596–1628` | `ZSTD_generateSequences` is explicitly a deprecated, imperfect debugging API. It may fail for valid input. Its final row per block has zero offset and match length, including zero trailing literals. |
| `lib/compress/zstd_compress.c:3520–3552` | The collector rejects nonzero target compressed block size and workers. It runs `compress2` with sequence collection enabled and discards its generated frame. |
| `lib/compress/zstd_compress.c:4408–4412` | Collection copies candidate sequences, swaps previous/next block state, and returns before entropy compression. Thus it publishes candidates even when ordinary compression would choose raw/RLE. |
| `lib/compress/zstd_compress.c:4414–4440` | Ordinary compression runs entropy selection, may turn a non-first uniform block into RLE, and confirms previous/next state only for a successful payload size greater than one. |
| `lib/compress/zstd_compress.c:3606–3612` | Confirmation is a pointer swap of previous/next compressed-block state. Observe stable local owner labels, never serialize pointer addresses. |
| `lib/compress/zstd_compress.c:3273–3281` | A block smaller than seven bytes returns before resetting the sequence store. Do not report its stale store as a new candidate. Collector mode reports an error for this no-compress path at `:4401–4403`. |
| `lib/compress/zstd_fast.c:370–372` | Fast writes parser repeat cells zero and one only; cell two is untouched. This three-cell array is not the full decoder-visible repeat state after replay. |
| `lib/compress/zstd_compress.c:3469–3492` | The collector resolves explicit offBase intent to distance and independently replays all three repeat offsets. Preserve that distinction. |
| `lib/compress/zstd_compress.c:3500–3505` | The block-delimiter assignment does not initialize its `rep` field. Zero-initialize the collector output and canonicalize delimiter rep to zero; never compare an indeterminate value. |
| `lib/compress/zstd_compress_internal.h:126–139` | Decode sequence lengths with `ZSTD_getSequenceLength`, including the single long-length extension; reading the raw 16-bit members loses lengths at 64 KiB. |
| `lib/compress/zstd_fast.c:443–479` | minMatch three dispatches to the four-byte parser; five/six/seven select their matching key widths. Window logs below 19 use the cmov variant, 19 and above the branch variant. |

## Direct parser fixture setup and safety

Use a new test-only C helper, for example `tests/upstream/fast-parser/fast_oracle.c`,
including the pinned internal declarations and linking the unchanged pinned
static library. The exported `ZSTD_compressBlock_fast` entry point is sufficient;
there is no need to copy its body, remove `static`, or add logging hooks to it.

The harness owns one bounded source arena, one `U32` hash array, and one sequence
store. Suggested initial caps are 2 MiB logical source per case, 128 KiB per
block, hash logs 6–18, window logs 10–20, and target length at most 131072. These
are fixture admission limits, not claims that upstream supports only these
values. Require every extent, shift, count-times-element-size, and C `U32` index
conversion to pass explicit checked validation before calling C.

Use an initialized arena with at least 64 bytes before logical source and a
checked suffix covering the largest acceleration pointer, increment boundary,
prefetch formation, and hash-read slack. A conservative initial suffix of
`max_target_length + max_block_size + 512` bytes is bounded here. Verify its
adequacy from every pointer expression in `zstd_fast.c:192–422` during helper
review. Initializing only eight trailing bytes is insufficient: even before an
early cleanup, the C function forms `ip2 = ip0 + step` and `ip3 = ip2 + 1`.
The initialized suffix keeps those pointer expressions within one allocation;
the unchanged logical `iend` still limits eligible matches. Subsequent logical
blocks use the real following input bytes, not synthetic per-block padding.

Arrange `base` two bytes before the logical frame start, while still inside the
arena. Start valid C indices at two, set no-dictionary limits accordingly,
zero-initialize other match-state fields, set `loadedDictEnd=0` and
`dictMatchState=NULL`, and keep the entire logical input contiguous and live.
Invalid zero/one hash entries must remain invalid after normalization; do not
map C zero to Rust source position zero. Use C positions minus two for valid
frame coordinates, plus an explicit invalid-slot variant.

For directed preseed cases, each nonzero table entry must reference an earlier
initialized position with the corresponding hash bucket. Hash collisions are
valid; future positions, arbitrary out-of-range indices, and pointers into
different allocations are not valid C test inputs. Repeated offsets outside
the current prefix may be supplied as positive saved offsets, because the
unchanged parser explicitly disables/restores these before use. Do not inject
invalid caller state merely to test Rust's defensive error paths.

Reserve sequence capacity at least `block_size / MINMATCH + 1`; literal storage
has logical capacity `block_size` plus `WILDCOPY_OVERLENGTH` writable slack,
matching `zstd_compress.c:2242–2248`. Reset the store before each direct call.
`ZSTD_storeSeq` may write literal-copy slack even for a short literal run
(`zstd_compress_internal.h:776–810`). Keep guards outside that permitted slack.
Allocate and initialize any ancillary store arrays required by the chosen
initializer, and make their capacity checks explicit rather than relying on C
assertions. Preserve initialized source guards separately from literal guards.

The direct parser returns the number of trailing literals, without appending
them. Copy/check those logically in the harness after the call. Verify that
every LL/ML interval, resolved distance, and trailing region replays exactly the
input using an independent checked replay loop. Capture parser cells before and
after separately from full semantic repeat replay. Comparing Fast's first two
returned cells to replay is useful; requiring its untouched third cell to equal
replay is incorrect.

Run the future helper once with the normal pinned build and once with an
independently compiled sanitizer build. Assertions and sanitizers complement
the explicit bounds review; they do not replace it. Keep compiler flags,
library hashes and actual entry-point names in provenance. Do not claim these
checks have happened yet.

## What the observations can and cannot prove

The unchanged routine exposes emitted sequences and final hash-table state,
not a callback for each probe or store. Therefore call the result a
**sequence and state oracle**, not an instruction-by-instruction trace.

For hash log six, store all 64 before/after slots explicitly. For larger tables,
store a canonical little-endian full-table digest and a sorted list of changed
slots with old/new values; these must reconstruct the full final table from the
initial table. This is independent of host byte order. Record the exact arena
padding recipe for reproducibility. A terminal frame's unused table state may
depend on permitted padding reads; changing guard patterns need only preserve
logical sequences/output/repeats, not every unreachable final hash slot.

Collision-directed witnesses distinguish delayed lookup/store order through
their observable sequence and final-state consequences. They do not establish
that every overwritten intermediate store was independently recorded. If a
later review requires actual per-store traces, that is a separately disclosed
instrumented oracle, checked against these unchanged-code observations.

For every chosen recipe, freeze its exact bytes/parameters and C observations
before Rust Fast exists. Later run the same recipe through existing independent
Greedy as a negative control and retain at least one actual distinguishing
LL/ML/offset-intent or table-state witness. Do not invent a differing outcome
from source inspection alone; a recipe whose outcome coincides remains useful
for boundaries but cannot demonstrate algorithm discrimination by itself.

## Small deterministic recipe families

Use fixed byte recipes, fixed integer recurrences, and explicit overwrite
ranges; do not depend on the host RNG or C compression output to construct
inputs. The exact final case count should be fixed after C-only exploration,
before Rust implementation. A compact initial set can use these families:

| Family | Dimensions and required observable |
| --- | --- |
| Short tails | Logical sizes 0–16, then 23/24/25, 31/32/33; target 0/1/4/131072. Use ordinary frames for 0–6 and direct parser cases only where their setup is reviewed. Distinguish bypass, empty candidate, last literals, eight-byte search margin and first-position skip. |
| Hash widths | minMatch 3/4/5/6/7 with identical asymmetric bytes; hash logs 6 and 18. Three/four must share effective four-byte hash behavior. Include 5/6/7-byte differences beyond byte four. |
| Step growth | Nonmatching prefix with a planted short repeat around paired candidate positions near 127/128/129 and 255/256/257; target 0/1/2/3/4/5 and 131072. Require observed skipped/probed-state distinctions, not only round trips. |
| Pipeline collisions | Small hash table, deterministically searched colliding asymmetric keys, valid preseed matches for first/second paired probes. Freeze explicit collision tuples; compare lookup-before-store and the conditional next-slot write at step four versus five. |
| Match extension | Planted matches with zero, one and several matching preceding bytes, with anchor and prefix-bound stops. Separate the rep path's one-byte extension from ordinary offset backward extension. |
| Immediate repeats | Alternating repeated pieces yielding zero-LL repeat sequences after an ordinary match. Preserve offBase one with LL zero, actual raw distance, and both parser-used repeat cells. |
| Duplicate saved offsets | Valid repeat arrays such as [4,4,8], plus distinct arrays; explicitly distinguish a normal offset from a semantically equal repeat. Source is an initialized prior prefix plus current block. |
| Disabled repeats | At entry, zero/one/two saved offsets exceed the current prefix; cases produce no new offset, one new offset, or two. Verify cleanup restores positive cells and uses the special saved-first-to-second rule. |
| Lower bounds | Seed a prior candidate at block-end-derived prefix lower bound minus one/equal/plus one. Separately test repeat distance at current-position maxRep and maxRep+1. These are different boundaries. |
| Variant threshold | Equivalent valid state at window logs 18/19, with the same effective lower bound, exercises cmov/branch dispatch without conflating different windows. |
| Long lengths | Literal run and match length around 65535/65536 and through 128 KiB. Decode the store's long-length metadata, not truncated members. |
| Persistent blocks | Fixed contiguous 2–4-block recipes: incompressible block followed by copied source, compressible block followed by uniform RLE then repeat-bearing data, and source-window expiry. Compare actual candidate tables, prior/next state and final block headers. |

The parser's fixed block-end lower bound is from `zstd_fast.c:203–205` and
`zstd_compress_internal.h:1412–1425`. Initial repeat availability instead calls
the helper at the current position (`zstd_fast.c:239–243`). Avoid a single
generic per-probe distance rule in either oracle normalization or Rust tests.

## Collector corroboration protocol

Create a fresh CCtx, set all seven scalar compression parameters explicitly,
choose Fast, and disable other sources of sequence production. Explicitly set
workers zero, LDM disabled, target compressed block size zero, pre-split level
one and post-split disabled. Record the actual applied scalar record after
initialization: public parameter resolution may adjust small inputs, and those
adjustments must not be mistaken for direct-parser behavior.

Use checked `ZSTD_sequenceBound` and zero-initialized output. Preserve error
name/code outcomes rather than pretending every valid source succeeds. For
success, verify delimiters, complete source coverage and independent sequence
replay. Compare a direct campaign using the same actual blocks and candidate
publication at every block. If an ordinary-frame campaign rolls back a block,
divergence from this collector trajectory is expected, not a Rust bug.

This only adapts a Fast subset of the sequence APIs. It does not complete
`tests/fuzz/generate_sequences.c`, whose producer chooses broader parameters,
varies output capacity, invokes sequence compression and merges delimiters.
Likewise `tests/fuzzer.c:3764–3818` uses level 19 and includes separate
small-capacity assertions; do not mark that whole group ported by Fast traces.

## Ordinary frame publication protocol

Use a second fresh context initialized with complete valid parameters and no
dictionary through the pinned begin initializer. A test-only helper may use
`ZSTD_compressBegin_advanced_internal` (`zstd_compress.c:5285–5300`) so all explicit
splitter/worker/LDM controls survive. Verify `appliedParams` after initialization;
do not assume requested values remained unchanged.

Supply the same stable contiguous arena one actual block per
`ZSTD_compressContinue` call, with each block no larger than the context's
effective block size. Keep pre/post splitting disabled. After each call, before
the next call can overwrite scratch, record:

- source block range, actual frame/block header kind, produced payload bytes;
- previous/next repeat cells before and after, normalized previous-owner swap;
- current candidate sequence store and independent full wire replay, only if
  the source was large enough to construct a fresh candidate;
- canonical match-table state and the source/window bounds used for this block;
- committed entropy/tree identities or deterministic logical fingerprints where
  exposed, without serializing padding or native addresses.

Finish with `ZSTD_compressEnd` and decode the complete frame independently with
the pinned ordinary decompressor. Empty final blocks are recorded explicitly.
Using frame mode is important: raw `ZSTD_compressBlock` passes frame=0
(`zstd_compress.c:4883–4889`) and therefore does not exercise the frame-only RLE
selection at `:4424–4432`. Merely testing the raw block API misses that path.

For comparability to whole-buffer mode, also compress the full contiguous
source with the same actual parameters, verify its block boundaries and decode
it. Establish equivalence experimentally for each retained recipe; chunking
and requested/applied settings are metadata, not assumed interchangeable APIs.

The first block is intentionally not emitted as RLE in this pinned normal
path (`:4424–4432`). Include a prior nonempty block before claiming an RLE
publication witness. Raw/RLE retain sampled match history but do not publish
pending wire repeats/entropy. A candidate whose repeats happen not to change
is insufficient alone to prove rollback; require an observed differing
candidate and prior state, or explicitly qualify that case.

Do not strengthen this into “every committed entropy metadata byte is
unchanged”: `zstd_compress.c:4444–4445` changes an existing offset-table repeat
mode from valid to check after the selection, including a raw/RLE result.
Record that metadata transition independently from candidate-table publication.

## Stored records, provenance and bounded validation

Records are plain values, owned fixture bytes and index/range descriptions.
No artificial unsafe backend is appropriate for this test metadata. Suggested
schema: one manifest plus per-case JSON (or TSV with explicit fields), source
bytes/recipe, normalized sequence rows, small-table values or full-table hash
and changes, and ordinary frame assets only where needed for replay.

Each manifest should bind reference revision, clean source hashes, helper and
generator hashes, build command/environment, compiler/version, target pointer
width/endian, complete scalar parameters, disabled feature controls, block
schedule, allocation caps, arena-padding recipe, file sizes/hashes and case
counts. Canonical serialization must reject duplicate case IDs, missing final
rows, incomplete byte coverage, inconsistent counts and unexpected source
changes. Logical sequence/table fingerprints must not include C structure
padding or host addresses.

The normal offline Rust tests should validate saved C values without a C
dependency. A separate explicit oracle test regenerates every case using the
pinned checkout and byte-compares canonical records. Keep sanitizer and normal
build observations distinct. Record exact commands and positive case counts;
do not permit a zero-case regeneration to pass.

## Suggested small checkpoints

1. Design/data-structure/TODO entry for the three oracle trajectories and
   metadata schema; no codec implementation yet.
2. Bounded direct C helper with a few hand-readable short/collision/rep cases,
   independent replay and padding/capacity safety checks; freeze source and
   normal/sanitizer regeneration evidence.
3. Complete deterministic direct fixture families and canonical state schema,
   including explicit offset intent and separate repeat triples.
4. Collector corroboration plus documented tiny-input/errors/delimiters.
5. Ordinary frame publication fixtures for compressed/raw/RLE transitions,
   including applied parameters and full independent decode.
6. Only then implement Fast and consume the saved sequence/state fixtures;
   record an actual distinguishing existing-Greedy negative control. Reuse the
   current safe/unsafe hash-table backend comparison, including every active
   feature separately and all together.

Dictionary Fast, extDict/dictMatchState paths, index-overflow rescaling,
noncontiguous streaming, workers, LDM, other reference strategies and full
sequence-API migration remain outside this first no-dictionary whole-buffer
scope. Frame interoperability and reference parser decisions are meaningful
separate claims; C-identical entropy bytes, ratios or speed are not implied.
