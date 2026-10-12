# Ordinary DoubleFast helper review before first execution

The coordinator inspected `scripts/oracles/double_fast_frame.c` against the
frozen ordinary-frame protocol and pinned upstream source on2026-10-12, before
running any helper mode. Reviewed helper SHA-256:
`90e8a70d27bc79d36443033659918fc5458ad70dc8b8006277e39f04168a22dd`.
Strict C11 syntax/diagnostic checking passes. This review is not a C capture or
proof that the proposed recipes produce their intended publication witnesses.

The helper adapts the project's frozen Fast frame observation scaffolding and
calls the unchanged v1.5.7 library. It copies no parser or entropy algorithm.
Complete explicit DoubleFast parameters are checked before context construction;
window21 and independent long/short dimensions are admitted without clamping.
Applied context and match-state parameters must equal the requested tuple.
The observed table pointers must be nonnull, independently owned and aligned.
Their initialized lengths follow the pinned context allocator using those
verified dimensions; each before-copy uses its own checked element count.

Seven persistent guarded buffers are live during scheduled frame capture:

| Owner | Payload bound and use |
| --- | --- |
| Source arena | N+64+262656 initialized bytes, N<=4MiB; source begins64 bytes into the payload. |
| Source copy | Same full payload, compared including leading/suffix initialization after calls. |
| Scratch output | 2MiB. Each call checks the conservative literal/sequence/entropy upper bound before compression. |
| Complete frame | N+3*(positive calls+1)+maximum header/checksum overhead, checked before every append. |
| Independent replay | max(N,1), initialized; every literal/match end is bounded before copying. |
| Long before-table | sizeof(U32)*2^H, H6..18, explicitly checked multiplication/alignment. |
| Short before-table | sizeof(U32)*2^C, C6..18, independently checked multiplication/alignment. |

Each includes two64-byte guards outside the full permitted payload. At N=4MiB,
64 positive calls and both log18 tables, these helper allocations total at most
21,498,073 bytes including guards. C context/workspace allocations are separate
owners governed by checked parameters, not included in that helper-byte figure.
Complete decoding temporarily adds max(N,1)+128 bytes. Aligned whole comparison
adds its checked `ZSTD_compressBound(N)` buffer, a separate C context and another
decode buffer; temporary owners are released before main-buffer cleanup. No
whole-program exact allocation total or error-path leak instrumentation is claimed.

The logical source remains one allocation for the whole schedule. The first
positive ordinary input establishes source-minus-two as the C base; this remains
inside initialized leading space. The helper verifies contiguous window/base,
no attached dictionary and no overflow correction after each positive block.
The suffix exceeds the separately reviewed noDict DoubleFast pointer-formation,
selected/physical key-read and wildcopy needs; target is not a DoubleFast step.
Later source blocks are real initialized bytes, not substituted per-block slack.
All logical indices stay below4MiB+2 and fit U32 on admitted32/64-bit hosts.

At `zstd_compress.c:4630`, `ZSTD_window_enforceMaxDist` receives the block-start
`ip`, despite the helper function's parameter name `blockEnd`. Retained ordinary
context floor therefore differs from the parser's block-end candidate floor.
The validator preserves this distinction and accepts stale table cells below
the active floor when their stored key still lies in consumed logical source.
Both complete maps are copied/reconstructed independently, including tiny paths;
zero calls explicitly verify neither table changed.

Sequence/literal pointers are read only from initialized context storage after
a fresh size>=7 candidate. Integer extent checks precede iteration; upstream
`ZSTD_getSequenceLength` expands the long field. Every observed match has length
at least4 and offBase1 or explicit distance+3. Replay checks available literals,
positive distance, current output, retained floor/window, and block end before
overlapping byte copies. Scratch/literal counts and every full frame append are
bounded independently of codec success. The raw parser's third repeat is kept
separate from the complete wire replay; only compressed blocks commit the trial.

Pending block state is never inspected before initialization. Snapshotting HUF
widths occurs only with an initialized non-none repeat mode, checked table log
and alphabet. Owner labels are local0/1 values, not serialized addresses. Raw/
RLE state retention, first-block RLE exclusion, tiny stale generations and empty
Continue publication are checked from actual ordinary context behavior.

Output files use exclusive creation. The future driver additionally must reject
input/output/reference/repository overlap before creating its fresh directory,
record exact file/source/header/compiler identities and bounded logs, and compile
all used C in normal and sanitizer modes. Partial-header probes deliberately
allocate no pledged source, issue only zero Continue and abandon the context;
they are labeled incomplete and undecoded. No collector call is present.

The independent Python validator reuses only frozen generic frame/header/state
inspection helpers after checking their identity. Its new source replay, exact
DoubleFast offset intent and two-table dimensions/hashes are explicit. Tests of
that validator use frozen hash vectors and clearly synthetic structural records;
those inputs are not represented as observed DoubleFast frames. The full
ordinary-frame semantic checks will be exercised against subsequent C capture,
with all initial mismatches retained for review.
