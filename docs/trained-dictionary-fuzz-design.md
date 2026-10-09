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

## Completed adapter checkpoint

`tests/upstream_trained_dictionary_fuzz.rs` contains 13 ordinary tests (including
five shared producer checks), an opt-in independent C selector oracle, and an
opt-in external corpus runner. Normal tests passed safe, all-feature and
no-default-feature configurations; strict all-feature Clippy also passed.
The C oracle verifies all 3,600 selected ranges and remaining producer positions
for 36 cases. Dictionary modes, prepared/loaded/prefix selection, empty fallbacks,
bounded destinations, malformed input, successful controls and streaming modes
have executable assertions.

`dictionary_decompress.c:27` preserves every selector before decoding. Prepared
and loaded dictionaries use the same validated shared Rust handle; the load-
method selector is consumed but borrowing is replaced by owned sharing. Its
output bound is selected from zero through ten times the source prefix length.
Streaming progress and canaries adapt the original bounded one-shot destination.

`dictionary_round_trip.c:29,110` preserves reserved source bytes, the reduced
compression bound, training sample offsets, prefix and content-mode selections.
The legacy compression level is consumed but default Rust compression is used.
Advanced parameter consumption and subsequent load-method selectors are omitted;
those later offsets are not claimed equivalent to the original producer state.
Repeated compression must produce identical bytes, and decoding must reproduce
the complete source.

`dictionary_stream_round_trip.c:149` preserves its pre-training mode, sample and
prefix selectors. Its mode/flush/finish and input/output selection order follows
the original state machine after the unsupported random-parameter call. Because
that call's consumption is omitted, the initial schedule position is adapted.
All pending input/output is drained, every frame completes, and repeated frames
retain persistent dictionaries while consuming a prefix only once.

The full matching corpora passed safe and all-feature release builds without
skipped files. An expected fallback occurs only for `TrainingFailed`; allocation
and unexpected errors cannot silently replace a dictionary or skip an input.

| Target | Inputs per build | Trained dictionaries | Empty fallbacks | Successful decodes | Rejected compressed inputs |
| --- | ---: | ---: | ---: | ---: | ---: |
| `decompress` | 12,063 | 11,768 | 295 | 20 | 12,043 |
| `round_trip` | 30,925 | 30,722 | 203 | 30,925 | 0 |
| `stream_round_trip` | 24,223 | 23,821 | 402 | 24,223 | 0 |

These are 67,211 selected target inputs per build and 134,422 backend replays in
total. Each source is limited to 2 MiB; traversal allows at most 65,536 files,
rejects links/special files, and retains a maximum depth of 64. Decoder windows
are capped at 128 MiB. Roundtrip decoded output must equal the source length;
streaming compressed scratch is capped at fifteen times the source compression
bound. All declared archive members fit these limits; limits do not truncate
inputs.

The following external logs record complete per-file runs. Elapsed times describe
these test executions and are not safe/unsafe throughput measurements.

| Log basename under `/tmp` | Seconds | SHA-256 |
| --- | ---: | --- |
| `zstdoxide-trained-decompress-safe.log` | 11.754 | `f7aa5507114f656b6f7adbf1827561b3995fafcec47d44a3bb41ba3eaef00700` |
| `zstdoxide-trained-decompress-all.log` | 11.791 | `e5e90e52b861386d182174d2f4b8eeb089b228a7e5720def8f5a812b9a487a72` |
| `zstdoxide-trained-round_trip-safe.log` | 41.388 | `f987af85d180e3e0aa3dcaca2c87b85fe9e16d23b3fb3dba50374ca071c32bd7` |
| `zstdoxide-trained-round_trip-all.log` | 40.502 | `163eb5df92ab4e5a878b98a3ba68b5b643967e23c545c7d459dab7ebd8559c0e` |
| `zstdoxide-trained-stream_round_trip-safe.log` | 42.511 | `d9aba51900f643d8ef7d404aee4a9004aa3aad408d1bb27d5f6840dc7f538b9d` |
| `zstdoxide-trained-stream_round_trip-all.log` | 46.449 | `e0fe41065b0fec611ca6358154b4a7a963bdcd1669e86753dcda10949b781700` |

Reproduce one target, then repeat with `--all-features`:

```sh
ZSTDOXIDE_DICTIONARY_CORPUS=/path/to/cache/extracted/dictionary_decompress \
ZSTDOXIDE_DICTIONARY_TARGET=decompress ZSTDOXIDE_DICTIONARY_MAX_FILES=65536 \
  cargo test --release --test upstream_trained_dictionary_fuzz \
  external_trained_dictionary_file_corpus -- --ignored --nocapture
```

Select `round_trip` with the `dictionary_round_trip` directory or
`stream_round_trip` with the `dictionary_stream_round_trip` directory for the
other targets. All three upstream source mappings remain partial because
fastCover and the missing C parameter/control branches have not been ported.
