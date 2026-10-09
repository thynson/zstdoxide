# Trained dictionary fuzz adapters

This decomposition precedes the three training-dependent v1.5.7 fuzz adapters:
`dictionary_decompress.c`, `dictionary_round_trip.c`, and
`dictionary_stream_round_trip.c`. Their common helper is
`tests/fuzz/zstd_helpers.c::FUZZ_train`. Production training remains independent
Rust code; the upstream fastCover algorithm and unsupported compression
parameters remain explicit partial-coverage obligations.

## Sample selection and ownership

The existing checked `DataProducer` retains the exact original tail-consumption
semantics. Each target first reserves its source prefix, then consumes its own
pre-training selectors in upstream order. The helper chooses dictionary capacity
`max(source_size / 8, 1024)` and aggregate sample capacity eleven times that size.
For each of exactly 100 samples it consumes an offset in
`0..max(source_size, 1)-1`. The selected length is the smaller of remaining source
bytes and `remaining_sample_capacity / remaining_sample_count`. Even a zero-byte
sample consumes its offset selector. Maintain the aggregate selected length with
checked arithmetic; no sample may leave the source prefix or total capacity.

Store sample boundaries as index ranges or resolve temporary shared slices into
the original immutable fuzz source. Neither dictionary nor context retains these
borrows. The sample descriptors are ordinary test-only value records with no
unsafe alternative. Independent C-generated vectors must establish the selected
ranges and post-training producer position, including empty and exhausted input.

Call the formatted Rust trainer with the same dictionary capacity and enough
sample budget to observe every selected byte. Training failure selects an empty
raw dictionary, matching the target's fallback contract. Do not turn allocation,
format, or unexpected internal errors into a silently skipped fuzz input. Empty
dictionary content disables selection even when the consumed requested mode is
Formatted, corresponding to the upstream zero-size context-loading contract.
Report successful training and expected fallback counts separately.

## Target contracts

1. Decompression preserves dictionary/prepared/prefix mode selectors and the
   selected output capacity `0..10*source_size`. Malformed data may return an
   error; every success remains within bounds and reports consistent progress.
   Include deterministic valid-frame controls so the suite cannot pass solely
   by rejecting inputs. Owned immutable sharing replaces C by-reference loading.
2. One-shot roundtrip preserves the source prefix, the one-byte-smaller
   compression-bound choice, training, dictionary modes, and prefix choice.
   Disable the checksum for the reduced-bound contract; compare exact decoded
   bytes and repeated encoded bytes. Unsupported level/random-parameter paths
   must be documented rather than claimed as covered by default options.
3. Streaming roundtrip uses the original training/mode/prefix setup and adapts
   the Continue/Flush/Finish schedules to the public Rust progress contract.
   Consume and produce only the reported prefixes, check destination canaries,
   finish every frame, and retain persistent dictionaries across reset. A prefix
   is consumed by only the first standard frame. Preserve source order across
   concatenated frames, including empty calls and fragmented destinations.
   Exact upstream randomized parameter consumption remains separate until those
   parameters exist; state which scheduling selectors are adapted.

## Replay and completion evidence

Each target gets deterministic boundary, noise, and successful controls before
an opt-in external replay. Reuse bounded, sorted, non-symlink traversal and reject
oversized inputs explicitly. Replay the matching complete pinned corpus under
safe and all-feature release builds. Record file counts, training/fallback
counts, successful decode counts where relevant, limits, elapsed times and log
hashes. An archive shared with another target does not establish its coverage.
Update source/case mappings only after executing the referenced tests. These
adapters are deterministic assertions, not a coverage-guided fuzz campaign.
