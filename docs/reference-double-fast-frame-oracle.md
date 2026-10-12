# Ordinary DoubleFast frame observation protocol

Prepared after the independently frozen direct observations and single-block
Rust parser. This document defines a separate ordinary-frame capture. Its source
recipes are input hypotheses, never expectations derived from the Rust parser.
No ordinary DoubleFast capture is claimed by this planning checkpoint.

The reference remains facebook/zstd v1.5.7, revision
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`, outside the repository. Source identities
are pinned by the existing inventory and DoubleFast design. Reuse the reviewed
Fast frame helper's setup/observation design in a new helper; do not modify its
frozen source, support modules, recipes or observations.

## Entry points and distinct states

Use `ZSTD_compressBegin_advanced_internal` with complete explicit parameters,
then ordinary `ZSTD_compressContinue` calls matching the fixed schedule and an
empty `ZSTD_compressEnd`. Keep content size enabled, checksum/dictionary/workers/
LDM/external sequences disabled, splitting disabled, and max block size128KiB.
Admit W10..21, H/C6..18, S1..29, M3..7 and T0..131072. Set strategy DoubleFast.
Require every requested scalar to equal both applied parameters and match-state
parameters before observing parser state. These are internal explicit-parameter
observations, not public parameter-resolution evidence.

Keep stable local labels for the two compressed-block state owners; never
serialize addresses. Read only the initially initialized previous owner or a
pending owner after a successful fresh candidate. Lengths1..6 retain stale
sequence/literal scratch and must not be reported as new candidates. Lengths7+
create candidates even if they emit no sequences. Zero-size Continue preserves
publication and source progress; record whether it emits the initial header.

For each positive block record source range, actual block header/payload,
first-block flag, previous/pending owner labels, committed entropy tags and HUF
widths, candidate freshness, complete expanded sequences/offBase/distances,
literal bytes/trailing length, candidate raw repeats and independently replayed
full wire repeats. Confirm owner swap exactly for compressed blocks. For raw
and RLE verify committed repeat state and HUF state retention, with the existing
source-qualified offset-tag demotion exception. Track both sampled tables
independently of whether the candidate is committed. Do not compare raw third
repeat cells to the semantic third repeat.

No collector API participates. Optional later `ZSTD_generateSequences`
corroboration must retain its unconditional candidate publication and long-LL
qualification as a separate capture, not an ordinary-frame oracle.

## Bounds and ownership review before execution

Logical source is one contiguous immutable allocation, capped at4MiB, with
64 initialized leading bytes and a conservative262656-byte initialized suffix
(128KiB target allowance +128KiB block +512). The noDict DoubleFast search does
not consume target as an acceleration step, so this exceeds the direct helper's
reviewed suffix requirement. Source+2 index normalization stays below U32 limits;
no near-4GiB index-reduction behavior is claimed. C owns and initializes its own
context workspace; do not construct or seed its tables manually.

The first normal input call establishes base=source-2 inside the same source
arena. The observation helper must verify the expected no-dictionary contiguous
window, retained floor, zero loaded dictionary and zero overflow corrections.
Every eligible long8 or short max(4,M) key must end within consumed logical input.
Final stale entries below the current retention floor remain observable; do not
incorrectly require every retained cell to remain an eligible match.

The helper owns guarded source and complete source-copy allocations, a bounded
2MiB scratch destination, a complete-frame accumulation buffer sized to the
source plus every scheduled block header and maximum frame overhead, a full
replay buffer, and **two independent** initialized before-table copies. Each
table copy has checked `2^log * sizeof(U32)` bytes and verified alignment. The
context owns both table arrays, sequence/literal capacity and entropy state.
The helper must verify those dimensions/initialized extents before copying or
iterating, especially when H and C differ. Decode and aligned whole-buffer
comparisons create separately bounded temporary owners and free them before
completion. Guards surround the full allowed initialized regions, including
source slack. Review actual allocations and pointer arithmetic in the finished
helper against this description before its first C execution.

Schedules contain at most64 calls, each0..128KiB and at most the applied block
limit. Their checked sum equals the source length. A positive Continue must
produce exactly one nonfinal block; an empty End contributes a terminal empty
raw block. Reserve the frame bound before appending. Keep output paths disjoint
from input/reference/repository/report paths, require new output files, and
reject trailing packet/file data. Header-only probes have a known pledge below
UINT64_MAX and no allocated pledged source; abandon that context after reading
its header and label the result incomplete/undecoded.

## Two-table and sequence observations

Before each call copy each complete initialized table. After it, record each
table's independent log/width/count, complete canonical LE32 FNV fingerprints
and sorted bucket/old/new changes. Begin from C's observed zero tables, check
all old values, and reconstruct every cell independently using exact attributed
hash4..8 arithmetic. Raw index zero remains invalid; nonzero indices must be at
least2 and map to a selected-byte key wholly within consumed source. Canonical
deltas describe final state, not intermediate write chronology. Zero and tiny
calls must preserve both maps unless actual source inspection establishes a
specified exception; record any failed hypothesis instead of relaxing it silently.

Replay the candidate using expanded lengths and a separate three-repeat wire
state. Bound every literal/match range, positive distance, window and retained
prefix before overlap-aware copying. Compare every byte with the original
source. Only the actual compressed-block decision publishes the trial wire
triple. Both C and an independent Python validator perform this replay; neither
calls the Rust parser to determine expected values. Later Rust persistent-state
tests receive the recorded incoming committed repeats and publication schedule.

Decode every completed C frame with the unchanged reference and compare all
source bytes. For explicitly aligned full-block schedules and first uniform
input, also use `ZSTD_compress_advanced_internal`, decode that complete frame,
and compare per-block types/payloads while accounting for the scheduled terminal
empty block and final-bit difference. Arbitrary flush schedules make no such
whole-buffer equivalence claim. C and Rust entropy may choose different blocks;
public bidirectional interoperability remains a separate later work item.

## Frozen input families and capture discipline

The source-only generator defines51 recipes: empty/zero calls, first uniform,
compressed/uniform/copy transitions, incompressible copy/fallback hypotheses,
tiny stale/fresh boundaries, literal entropy with positive target and search
controls, short widths3..7, asymmetric low/maximum table dimensions,12 directed
raw-candidate hypotheses, and window17/19/20/21 retention/expiry candidates.
Ten fresh lengths1..10 and four prefix lengths7..10 isolate entry boundaries.
New larger window inputs extend beyond1MiB and2MiB. No block-kind or discarded
repeat witness is assumed before running the ordinary C helper. Preserve any
unmet hypothesis and add a separately pinned refinement only when necessary.

Header-only probes retain content-size field boundaries255/256,65791/65792,
UINT32_MAX-1/UINT32_MAX/UINT32_MAX+1, and add the W21 single-segment transition
at2097152/2097153. They do not claim full multi-gigabyte execution.

Capture normal and ASan/UBSan builds from unchanged common/compress/decompress
translation units, with assembly and legacy support disabled consistently.
Record every compiled source/header identity, compiler/flags/environment,
executable hash and exact command. Instrument the helper and every used C codec
translation unit in the sanitized build; disable LeakSanitizer explicitly.
Do not attribute sanitizer coverage to an uninstrumented support archive.
All commands have bounded time/output and preserve failure/timeout/OSError
logs. Verify all pinned inputs before and after the campaign; reject destination
overlap before creating files. Require full normal/sanitized observation and
frame equality on the capture host. Offline checking executes no subprocess and
validates an independently pinned artifact catalog, exact JSON types/shapes,
source/schedule hashes, sequence/state/table replay and frame boundaries.
Add malformed-input and forged-observation rejection controls under normal
Python and `-O`. Record actual coverage and limitations instead of declaring
all intended witnesses successful from their recipe names.
