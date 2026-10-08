# Streaming conformance plan

Reference: facebook/zstd v1.5.7, revision
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`. Source identifiers below use the
`path:line` identifiers in `tests/upstream/manifest.json`; helper functions not
separately inventoried are identified explicitly. This is a design and test
decomposition, not a report that the listed tests pass.

Module 6 introduces reusable streaming contexts with bounded input staging,
bounded output staging, and a bounded history window. Its initial scope excludes
dictionaries, compression levels, stable input/output buffer modes, overlapping
input/output, worker threads, and caller-supplied workspaces. Source groups that
also assert these features remain partial even after their ordinary streaming
branches pass.

## Test data structures and ownership

The test driver owns source, compressed, and recovered bytes in `Vec<u8>`.
Each invocation borrows an input suffix and an independent mutable output slice;
positions are checked `usize` indices. No input pointer is retained across a
call. A `ChunkSchedule` consists of a deterministic seed, requested input/output
lengths, and a call count; it is a plain value record shared by all backends.
A `Transcript` records consumed/produced counts, completion events, and accepted
source ranges. A directive sequence records Continue, Flush, and Finish events.
These are ordinary test metadata, with no artificial unsafe variant.

Every driver checks `consumed <= input.len()` and `produced <= output.len()`
before moving its indices. It resubmits unconsumed input, including while a
Flush or Finish is pending. Output storage is freshly filled with sentinels;
the portion after `produced` must remain unchanged. A finite call budget detects
implementation livelocks without treating deliberate zero-capacity calls as
progress. After a scheduled zero-capacity call, the driver supplies the resource
requested by the returned status.

Production ring history, incremental checksum state, frame phases, pending
directives, and their safe/unsafe invariants are specified separately in
`docs/data-structures.md` and the module-specific design documents. Test drivers
exercise each selected production backend through the same public contract.

## Progress and lifecycle contracts

The adopted public API is specified in `docs/streaming-design.md`: decoder EOF
requires at least one completed frame and seals the context; errors poison it
until reset. Encoder completion requires an explicit reset. Pending Flush or
Finish must be resumed with the same directive. Encoder reset clears its pledge
while preserving the window, checksum, and content-size-emission options. The
following upstream concerns informed those choices.

1. `process(input, output)` reports exactly the bytes consumed and initialized.
   A need-input/need-output status describes the next required resource; a
   completion status describes a completed protocol event. Zero-length slices
   are valid, including both slices empty. Avoid translating the C return-value
   hints directly into the Rust status enum.
2. Decoder frame completion occurs after the last block, pending output, and
   optional checksum are all consumed or delivered. It stops at a frame boundary
   and leaves a following frame or invalid suffix unconsumed. Skippable frames
   also produce a completion event with zero output.
3. Decoder EOF validation is separate from temporary input exhaustion. EOF in
   a magic/header/block/payload/checksum/skippable payload is truncation. EOF at
   a clean boundary succeeds. The API must specify whether a never-started empty
   stream is a clean boundary, and whether pending output must be drained before
   EOF validation can succeed.
4. Raw block payload must flow incrementally from input to output. RLE blocks
   must keep producing after their one payload byte has been consumed.
   Compressed block staging is bounded by the format's block limit; the entire
   frame must never be retained merely to accommodate tiny caller buffers.
5. Flush consumes its offered source and makes every accepted byte decodable,
   without closing the frame or resetting its history. Retry Flush until the
   event is complete. Completion must not precede delivery of its last byte.
6. Finish consumes its offered source, closes the frame, and drains the epilogue.
   Continue during a pending Finish must be rejected or explicitly documented
   as a deliberate API difference. The caller retains unconsumed source between
   retries. The completed-frame policy must state whether new input starts a
   frame automatically or requires reset.
7. Reset cancels partial headers, blocks, buffered output, checksum state, and
   pending directives. Reset after an error makes the context usable again.
   Session reset preserves supported configuration, including the decoder's
   window limit. Parameter reset, if exposed, restores defaults independently.
8. `Some(0)` pledged source bytes means an empty frame; `None` means unknown.
   Excess accepted input fails promptly; Finish rejects insufficient input.
   Pledge accounting is independent of whether a content size is written.
   Upstream discards a pledge after the frame ends. Its immediate first-call
   End shortcut can override the pledge with the actual input size; a stricter
   Rust rule must be recorded as a difference, not claimed as exact parity.
9. A checksum error can follow already-delivered plaintext. The decoder must
   not announce frame completion until the checksum is validated. On error,
   output may already have changed; define a sticky failed state and require
   reset rather than implying transactional rollback.

The API reference for these choices is `lib/zstd.h:574-610`, `:728-823`, and
`:884-947`. Tests must assert the chosen Rust policy where the API intentionally
differs, while retaining the upstream group's partial status.

## Priority 1: exact bounded progress and frame completion

| Source group | Contract to preserve | Qualification |
| --- | --- | --- |
| `tests/zstreamtest.c:429` | Reject the impossible three-zero-byte magic prefix immediately. | Complete ordinary decoder contract; do not wait for a full four-byte magic. |
| `tests/zstreamtest.c:458`, `:495` | Incremental skippable/normal decoding with 0–16 byte input/output limits; exact final content, size, and consumed bytes. | Original normal frame is dictionary-backed; dictionary-free branch is an adaptation. |
| `tests/zstreamtest.c:526` | Empty encoder Continue/Finish with zero output, retry Finish to completion, decode the empty frame with zero output capacity. | Complete ordinary zero-capacity contract. |
| `tests/zstreamtest.c:554` | Continue accepts two bytes with zero output; repeated Finish drains a valid frame; decoder zero-output call is legal. | Tests bounded small-input staging, not arbitrary unbounded input acceptance. |
| `tests/zstreamtest.c:702` | An empty frame completes in a single call with zero output. | Stable-output mode and no-allocation assertion remain missing. |
| `tests/zstreamtest.c:1794` | A 10,000-byte raw block yields output while only 100 more compressed bytes are offered and output grows by 10 bytes. | Every inner call with pending payload input must produce bytes. A direct raw-frame fixture adapts the upstream negative-level encoder setup. |
| `tests/zstreamtest.c:385` | A skippable frame consumes its exact bytes and emits no output before the following frame is touched. | Setup uses a dictionary; skip behavior itself is independent. |
| `tests/zstreamtest.c:402`, `:412`, `:421` | Whole-frame completion, exact byte comparison, and decoding another frame on the same context without initialization. | Original data requires a dictionary; ordinary frame reuse is an adaptation. |

Add Rust-specific assertions around these groups: every proper nonempty frame
prefix is rejected by EOF validation, no checksum-free premature completion,
back-to-back empty/skippable/nonempty frames preserve exact boundaries, and a
large skippable payload is discarded without allocating its declared length.
An output-capacity matrix includes 0, 1, 2, 3, 7, 31, block-limit minus one,
block limit, and block-limit plus one. This is not a promise that all schedules
consume the same number of input bytes per call; only public progress and
completion invariants are compared.

## Priority 2: state preservation, reset, and limits

| Source group | Contract to preserve | Qualification |
| --- | --- | --- |
| `tests/zstreamtest.c:584` | A correct pledge is accepted and advertised by the frame header. | No compression-level byte-for-byte comparison. |
| `tests/zstreamtest.c:607` | A pledge one byte too large fails when finishing short input. | Preserve timing at Finish. |
| `tests/zstreamtest.c:625` | A pledge one byte too small fails when excess input is submitted. | Preserve rejection before successful frame completion. |
| `tests/zstreamtest.c:641` | A wrong pledge still fails with content-size emission disabled. | Requires a separate content-size flag; partial until exposed. |
| `tests/zstreamtest.c:657` | Reuse after 513-byte then 1025-byte frames recalculates per-frame state. | Original level-19/parameter behavior remains pending. |
| `tests/zstreamtest.c:1139` | Reject a frame whose window requirement exceeds the configured limit; reset recovers the context. | Original dictionary setup and full parameter reset are separate pending branches. |
| `tests/zstreamtest.c:1652` | Offset equal to window size and near-window offsets remain valid across history wraparound. | Original sequence-producer/window-log 10–26/29 sweep needs oracle fixtures or a sequence encoder; a small ring test alone is not this full group. |
| `tests/zstreamtest.c:1710` | Preserve match history across the exact 47-byte three-block regression frame with a 1024-byte window. | Check exact output in addition to upstream's successful-decode assertion. |
| `tests/zstreamtest.c:1743` | Decode the exact 51-byte three-block frame into the exact 60-byte expected output, preserving repeat entropy tables through tiny sequence sections. | All fixture bytes and expected bytes are in upstream source. |
| `tests/fuzzer.c:3641` | Parameter reset restores every available decoder parameter to its original value. | A window-limit-only API covers one adapted branch, not the C enumeration. |

Reset tests should interrupt each decoder phase and encoder directive, then
decode or encode an independent frame. Include reset after checksum error,
invalid magic, window rejection, and pledged-size failure. Repeat the same
source and call schedule before and after reset; deterministic compressed bytes
and decoded output must agree for the same supported settings. Retained memory
must stay bounded by window/block configuration rather than cumulative input.

`tests/fuzzer.c:1637` (reusing a compression context) exercises compression levels
1–19 and multiple APIs; only the same-context determinism principle applies to
Module 6. `tests/fuzzer.c:1577` exercises a size-unknown empty frame after Flush,
including bytewise decoding with zero output. Its level-1 initialization can be
adapted to the currently supported encoder strategy.

## Priority 3: directive schedules and corpus drivers

`tests/fuzz/stream_round_trip.c:156` owns the public fuzz entry point. Its
`compress` helper at line 59 selects input/output chunks from 1 to available
capacity and directives from a ten-way distribution: three Flush values, one
Finish/reset value, one empty Continue followed by Continue, and five ordinary
Continue values. It keeps the directive until Flush/Finish completes, including
when a source slice is only partly consumed. It can produce multiple frames
within a single original source. Its line-139 decoder consumes concatenated
frames. Port this state-machine behavior, compare the exact recovered prefix,
and verify that a completed Flush already exposes the source so far to a live
decoder before Finish is called. Random compression parameters, third-party
sequence producers, maximum-block-size variants, and overlapping decode remain
unimplemented; the complete fuzz source therefore remains partial.

`tests/fuzz/stream_decompress.c:68` reserves a data prefix through the existing
tail-driven data producer, reuses/reset a context, and selects arbitrary input
and output chunks including zero. The ordinary output branch uses a working
capacity of `max(10 * source_length, 128 KiB)`; exhausted parameter bytes switch
to all remaining input/full output. Malformed input may return an error, but
must not panic or exceed slice boundaries. Preserve unconsumed parts of a
selected input chunk. Stable-output mode is an explicit pending branch.

`tests/zstreamtest.c:2579` (`fuzzerTests`) generates five RDG distributions,
selects disjoint source fragments, randomly flushes, drains Finish in a loop,
and checks exact size/content and input consumption under independent decoder
chunking. It also mutates compressed spans and requires safe bounded error
handling. `tests/zstreamtest.c:2847` (`fuzzerTests_newAPI`) repeats the same
directive schedule twice and compares compressed checksums for determinism,
then verifies decoder exactness. Both groups also change dictionaries, levels,
parameters, and sometimes workers; the applicable dictionary-free schedules
remain partial adaptations of these large original campaigns.

## Deliberate differences and remaining contracts

`tests/zstreamtest.c:503` expects C to error after at most 100 calls with no
forward progress. A Rust need-output status can express this situation directly
without a hidden retry counter. Test that explicit status, but do not mark the
original no-forward-progress-error group ported.

Stable-output tests (`:798`, `:811`, `:825`, `:838`) assert fixed caller buffer
identity, immediate too-small-buffer errors, and context allocation size.
Ordinary output exhaustion returning need-output does not implement those
contracts. Buffered-output `:854` can be adapted, but its allocation-size
comparison still requires suitable accounting. Dictionary persistence/reset,
prepared dictionaries, prefix lifetimes, advanced initialization, multithread
flush, large-index overflow, and stable-input contracts remain open.

Before changing the main manifest, each implemented contract needs a real Rust
test identifier, passing focused tests, and a qualification for every omitted
assertion in the original group. Full sources with unsupported branches retain
partial status. No passing chunked one-shot adapter is evidence of bounded
streaming, incremental raw output, Flush visibility, or persistent context state.

## Initial Rust test mapping

All entries below live in `tests/upstream_streaming.rs`. The focused default
test run passed 39 tests (including five shared data-producer tests), with one
external-oracle test ignored by default. The ignored official v1.5.7 oracle
test passed separately with `ZSTD_BIN` set to the pinned checkout's executable.
The same 39 tests passed with all features enabled. Clippy with `-D warnings`
also passed. Additional Rust lifecycle tests
exercise the explicitly chosen poisoning, sealing, progress, and memory limits.

| Upstream identifier | Rust test | Scope qualification |
| --- | --- | --- |
| `tests/zstreamtest.c:1710` | `exact_history_overwrite_regression_survives_output_fragmentation` | Exact 47-byte fixture; all 1,063 output bytes checked against the official oracle. |
| `tests/zstreamtest.c:1743` | `exact_small_sequence_sections_survive_every_chunk_boundary` | Exact 51-byte fixture and exact 60-byte upstream expected output. |
| `tests/zstreamtest.c:1794` | `raw_block_outputs_immediately_under_ten_byte_backpressure` | Exact 100-byte availability/10-byte output behavior; direct raw frame replaces negative-level compression setup. |
| `tests/zstreamtest.c:385`, `:458` | `empty_and_skippable_frames_finish_with_zero_output`, `concatenated_frames_report_each_boundary_before_the_next_magic` | No dictionary setup; exact boundaries and ordinary buffered progress. |
| `tests/zstreamtest.c:429` | `invalid_magic_prefix_errors_early_and_requires_reset` | Original impossible prefix; Rust poisoning/reset assertion added. |
| `tests/zstreamtest.c:526` | `empty_encoder_accepts_zero_capacity_continue_and_finish` | Empty Continue/Finish and zero-output decode. |
| `tests/zstreamtest.c:554` | `encoder_accepts_two_source_bytes_before_any_output_is_available` | Original two-byte source, zero output, Finish retry, and legal zero-output decoder call. |
| `tests/zstreamtest.c:584` | `encoder_header_reports_pledge_window_and_checksum_options` | Supported encoder strategy and explicit advertised-window API. |
| `tests/zstreamtest.c:607`, `:625`, `:641` | `encoder_enforces_pledges_even_when_content_size_is_not_written` | Both error directions and both content-size flag values; supported strategy. |
| `tests/zstreamtest.c:1443` | `zero_pledge_accepts_only_an_empty_frame` | Typed pledge contract; C advanced initialization/dictionary branches remain absent. |
| `tests/fuzzer.c:1577` | `flush_of_empty_unknown_size_frame_does_not_close_the_frame` | Unknown-size empty Flush/Finish and zero-output decoding; no level-selection API. |
| `tests/zstreamtest.c:1008` | `flush_exposes_all_source_to_a_live_decoder_before_finish` | Original input ranges 18..100, 100..200, 200..518, with Flush in between; stable-input pointer contract absent. |
| `tests/zstreamtest.c:657` | `reset_clears_pledge_and_frame_state_but_retains_supported_options` | Original 513/1025-byte reuse sizes; level 19 remains absent. |
| `tests/fuzzer.c:1637` | `reset_repeats_identical_directive_and_chunk_schedules_deterministically` | Original 20 unique bytes followed by the first ten bytes; one supported strategy instead of levels 1–19 and all C entry points. |
| `tests/fuzz/stream_round_trip.c:156` | `mixed_directive_schedules_produce_exact_concatenated_frames`, `finish_resubmits_unconsumed_source_until_every_output_byte_is_drained` | Source-derived directive distribution, 22 source fragments, three mid-source finished frames, explicit reset, and exact recovered bytes; parameter producer, sequence producers, and in-place decode remain absent. |

`every_official_generated_frame_survives_independent_input_output_chunks`
replays all 96 preserved `decodecorpus` frames with (1-byte input, 17-byte output)
and (127-byte input, 3-byte output) schedules. Each schedule also injects empty
input/output calls, checks sentinels after the produced prefix, validates exact
content, and verifies completion at the exact compressed frame length.
The same test classifies the 16 header-only `.zst` assets separately, checks that
their headers are consumed without output, and requires EOF validation to fail.
Its output limit comes from each frame's expected original asset; the largest
preserved original is 107,098 bytes, above the common 64 KiB generator setting.

`streaming_fuzz_target_consumes_tail_selected_chunks_without_panics` uses the
shared exact `FUZZ_dataProducer` prefix and scalar-selection port. It runs 4,096
arbitrary inputs plus 3,200 schedules over five valid source-derived frames,
their truncations, and one-bit mutations. Input chunks and output capacities
include zero, selected source suffixes are retained until consumed, and output
sentinels and explicit progress bounds are checked. The original stable-output
selector is consumed, but its selected behavior is adapted to ordinary buffered
output. The adapter explicitly caps fuzz input at 65,536 bytes, cumulative
output at 1 MiB, and window size at 8 MiB. Exceeding a cap may return a resource
error; those cases are not evidence of full unbounded campaign parity.

`many_seeded_directive_schedules_preserve_data_and_compression_determinism`
runs 128 source/schedule seeds twice each, across supported windows and checksum
settings. Five deterministic content profiles include noise, one repeated byte,
a periodic pattern, a tiny alphabet, and sparse data. Source lengths include
0, 1, 1,023, 1,024, 1,025, and 131,073 bytes. Source chunks, output capacities,
and decoder chunks vary independently; Flush and Finish repeat until complete,
and Finish/reset can create multiple frames within one source. The second run
must emit identical compressed bytes. Bounded mutation and final-byte-truncation
checks reuse the streaming decompression fuzz driver. These are explicitly
adapted campaigns, not reproductions of the full upstream RDG/parameter/
dictionary/worker matrix.

The ignored oracle test
`official_codec_accepts_streamed_frames_and_stream_decoder_accepts_official_frames`
requires the official v1.5.7 CLI selected by `ZSTD_BIN`. It checks Rust output
after many Flush operations using the official decoder, then checks official
frames with the Rust streaming decoder. Empty, match-bearing, RLE-sized,
multi-block, and preserved HTTP inputs are included; checksum and window
settings vary. This verifies compatibility without promising identical
compressed bytes or unsupported compression parameters.

The window-distance group `tests/zstreamtest.c:1652` is implemented separately
in `tests/upstream_window.rs`. Its twenty official-oracle fixtures preserve the
original 10,000-sequence construction, fixed-seed `FUZ_rand` distances, and
every window logarithm from 10 through 29. The default small-window/tiny-output
tests and optional large-window sweep passed; the full sweep passed with safe
and all-features release builds. This covers the original decoder contract
without claiming that Rust exposes `ZSTD_compressSequences`.
