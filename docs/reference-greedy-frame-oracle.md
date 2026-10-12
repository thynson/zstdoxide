# Ordinary Greedy frame observation protocol

This separate oracle follows the frozen direct observations and checked Rust
direct parser. Input recipes are source-only hypotheses, independent of Rust
parser output. No ordinary Greedy C capture is claimed by this specification.
The reference is facebook/zstd v1.5.7, revision
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`, outside the repository.

## Calls and parameter scope

Use a fresh context initialized by `ZSTD_compressBegin_advanced_internal` with
explicit Greedy parameters, scheduled `ZSTD_compressContinue` calls and an empty
`ZSTD_compressEnd`. Content size is enabled; checksum, dictionaries, workers,
LDM, external sequence production/collection and block splitting are disabled.
Keep literal compression automatic and the maximum block size at 128 KiB.

Admit W10..21, H6..19, M3..7 and T0..131072. Request automatic row selection and
verify that the applied mode is enabled exactly when applied W>14. For chains,
admit C6..18 and S1..8; for rows, admit C6..29 and S1..29. The larger inactive
row chain log must not cause a chain allocation or a chain-table read. Review
`ZSTD_allocateChainTable` and `ZSTD_reset_matchState` before executing that case.
Require every requested cParameter to equal both the applied and match-state
parameter tuples. These internal explicit-parameter captures are not evidence
of public source-size parameter resolution.

Reuse the prior ordinary DoubleFast helper's guarded I/O, bounded lifecycle and
publication-observation design in new versioned sources. Preserve all frozen
Fast/DoubleFast helpers and evidence. The Greedy helper must distinguish the
selected family, owner element widths, source-coordinate chain-slot bias and
row head/tag geometry; renaming the old table labels is insufficient.

## Bounds, initialization and alignment

The logical source is one contiguous allocation of at most 4 MiB. Retain 64
initialized leading bytes and a conservative 262656-byte initialized suffix
inside its guards. Source index bias two remains in that allocation. No
near-4-GiB index reduction is exercised. Greedy's clamped key width is 4..6;
physical C reads/cache prefetches are separately permitted inside the initialized
suffix, while logical key membership must end within consumed source.

The context allocates its own workspace; the helper must not manually seed or
replace a compression table. Check independent initialized owner extents before
copying them. Chain heads/links contain 2^H/2^C U32 cells. Rows contain 2^H U32
positions and 2^H byte tags, including a reserved position/head cell in each
16/32/64-cell row. Hash row addresses must have 64-byte alignment; tag rows must
have row-width alignment for C prefetch/SIMD requirements. Allocate two separate
guarded before-copies with their actual element widths; never read a nonexistent
row chain owner. The C M3 hashTable3 allocation remains context-owned and is
not a Rust owner or C/Rust allocation-count equivalence claim.

Guarded helper owners include source/copy, scratch output at most 2 MiB, full
frame storage bounded by source plus scheduled headers and maximum frame
overhead, replay storage and the selected before-copies. Complete decode and
aligned whole-buffer comparisons use independently bounded temporary owners.
Review actual capacities, checked additions/products, guard extents, pointer
differences and failure cleanup in the finished helper before its first run.
Do not read an uninitialized pending compressed-block state.

At most 64 scheduled calls have lengths 0..min(128 KiB, 2^W, max(1, pledge)).
Their checked sum is the exact source length. Each positive Continue emits one
nonfinal block; an empty End emits the terminal empty raw block. Zero calls may
emit the first frame header but must not advance source/publication state.
Inputs and outputs must be disjoint from the reference/repository/report, output
files must be new, and extra input bytes/arguments reject. Header-only calls
use a known pledge below UINT64_MAX without allocating that pledged source;
label their output incomplete and undecoded.

## State and publication observations

Use stable local labels for previous/pending compressed-block owners, never
addresses. Record each call's source extent, frame/block bytes, first-block flag,
owner labels, committed repeat triple, entropy tags/Huffman widths and candidate
freshness. Inputs shorter than seven bytes bypass candidate construction and
retain stale scratch. For fresh candidates, record expanded and raw lengths,
wire offBase/distance, stored literal bytes, trailing length, the raw parser
repeat triple and independently replayed complete wire repeats.

Before/after every call record raw nextToUpdate, lazySkipping, row salt/entropy,
all eight C hash-cache cells and both selected complete table fingerprints and
canonical sorted deltas. For chains, normalize inactive row metadata explicitly
instead of reading uninitialized fields. The initial row salt must match the
previous fresh-context capture; unchanged salt and wrapping entropy are checked
through the whole trajectory. C cache contents are retained evidence, not a
required Rust data structure or an intermediate probe trace.

The ordinary wrapper adjusts nextToUpdate before a fresh parser call when
`current - next > 384`, setting it to
`current - min(192, current - next - 384)`. This applies to both families and is
separate from the row search's first96/last32 pending insertion rule. Tiny calls
bypass this adjustment. Record the actual wrapper input/output consequences;
do not relabel direct constructed-state observations as ordinary transitions.
Advance retained prefix eligibility at block start, while each Greedy search
still applies its own current-probe distance floor.

For every table delta, check log, element width, count, sorted unique cells, old
values and complete canonical fingerprint. Validate raw indices >=2, source
extents and key membership; permit retained stale cells below the current floor.
Chain link slots use `(logical_position+2) mod 2^C`. Row cell zero holds no source
position, its tag is an in-range head, and occupied tags agree with salted keys.
Source validation must not mistake canonical final deltas for insertion order.

Independently replay candidate sequences with overlap-aware copies, positive
distances within W and available retained prefix, and bounded literal ranges.
Only a compressed-block decision publishes candidate wire repeats and swaps
compressed-block owners. Raw/RLE retains search writes but discards candidate
repeats/entropy coding state, subject to the established source-qualified offset
tag demotion. Row hash salt entropy belongs to sampling and remains separate
from coding entropy. Zero/tiny calls retain only the source-qualified state.

Decode every completed C frame using the same pinned library and compare every
source byte. Explicitly marked aligned schedules also capture a fresh
`ZSTD_compress_advanced_internal` frame and compare block types/payloads after
accounting for final bits and the scheduled terminal empty block. Arbitrary flush
schedules make no whole-buffer equivalence claim. Rust ordinary parser tests
will consume recorded incoming committed repeats and publication decisions;
complete-frame/private entropy checks and public interoperability are separate.

## Input hypotheses and evidence discipline

Pin both chain/row families before running C: empty/zero calls, first uniform,
mixed compressed/raw/RLE candidates, incompressible then copied blocks, stale
tiny scratch, strict row/chain tail boundaries, M3..7, positive targets, pending
gaps383/384/385/575/576/577 and directed raw-candidate repeat changes. Include
S1/4/5/6/7/29 row layouts/budgets, asymmetric and maximum table dimensions,
inactive C29, skipping/reset and long literal/match candidates. Window14 chain
and W17/19/20/21 row trajectories include retention and expiry hypotheses.
Recipe names do not establish which emission or repeat-discard witness occurs.

Header-only probes preserve 255/256,65791/65792 and UINT32_MAX boundaries, plus
W14/15 automatic-branch and W21 single-segment transitions. They establish no
multi-gigabyte compression support. Every expected observation must originate
from unchanged C, never Rust-generated sequences or tables.

Build the helper and every used C common/compress/decompress translation unit
in normal and ASan/UBSan configurations, with consistent assembly/legacy/worker
exclusions and LeakSanitizer explicitly disabled. Pin all sources/headers,
compiler/flags/environment, commands and executable identities. Preserve known
upstream instrumentation exclusions and qualify coverage. Require identical
semantic observations/frame bytes across builds; allocator red-zone workspace
size differences retain both originals without claiming exact equality.

Capture uses bounded time/output, preserves failures, checks all pinned inputs
before/after and rejects path overlap before creating files. Offline validation
executes no subprocess, verifies an independently pinned artifact catalog and
exact JSON types/shapes, replays sequences, tables and publication, and checks
frame boundaries. Normal Python and `-O` must reject malformed packets and
forged observations. Keep unmet hypotheses as non-witnesses; any refinement gets
separate pinned inputs and evidence instead of rewriting the original recipes.
