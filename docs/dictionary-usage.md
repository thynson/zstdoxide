# Dictionary use and training

`Dictionary` owns its bytes and, for formatted dictionaries, validated immutable
Huffman/FSE tables and initial repeat offsets. Prepare it once, call
`into_shared()`, and reuse the handle with one-shot or streaming operations.
The handle uses `Arc` on targets with pointer atomics and `Rc` otherwise. Caller
input can be released after preparation; no borrowed address is retained.

## Raw and formatted dictionaries

```rust
use zstdoxide::{
    Dictionary, DecodeOptions, compress_with_dictionary, decompress_with_dictionary,
};

let dictionary = Dictionary::raw(b"content-type: application/json\r\n")?.into_shared();
let input = b"content-type: application/json\r\ncontent-type: application/json\r\n";
let frame = compress_with_dictionary(input, &dictionary)?;
let output = decompress_with_dictionary(&frame, &dictionary, DecodeOptions::new(1024))?;
assert_eq!(output, input);
# Ok::<(), zstdoxide::Error>(())
```

`Dictionary::from_bytes` detects the standard formatted magic when at least
eight bytes are available; otherwise it treats the bytes as raw content. A
recognized malformed formatted header is an error. `Dictionary::raw` always
treats all supplied bytes as content, even if they resemble a formatted header.
`from_bytes_with_mode(bytes, DictionaryMode::Formatted)` requires a complete
valid formatted dictionary. Empty and short raw content is accepted.

`as_bytes()` returns the complete serialization for storage or use with the
official codec. `content()` returns only match content, excluding formatted
metadata. `id()` is zero for raw dictionaries; `is_formatted()` distinguishes
the representation. Nonzero frame IDs require a selected dictionary with the
same ID: missing selection returns `DictionaryRequired`, and a different ID
returns `DictionaryMismatch`. An omitted or zero frame ID still uses the
configured dictionary, so callers must select the same content on both sides.

## Streaming selection and reset

```rust
use zstdoxide::{DecodeOptions, Decoder, Dictionary, Encoder, EncoderOptions};

let dictionary = Dictionary::raw(b"a reusable application-specific prefix")?.into_shared();
let mut encoder = Encoder::with_dictionary(
    EncoderOptions { include_dictionary_id: false, ..EncoderOptions::default() },
    dictionary.clone(),
)?;
let mut decoder = Decoder::with_dictionary(DecodeOptions::new(1024), dictionary);

// A session reset retains persistent dictionary selection and reusable buffers.
encoder.reset();
decoder.reset();
// Explicitly clearing selection also clears any pending single-use prefix.
encoder.set_dictionary(None)?;
decoder.set_dictionary(None)?;
# Ok::<(), zstdoxide::Error>(())
```

`set_dictionary(Some(handle))` selects a persistent dictionary. `set_prefix`
selects a dictionary for only the next standard frame; either call replaces the
other configuration. A pending prefix survives reset until claimed, and
skippable frames do not claim it. Once claimed by a standard frame, the prefix
does not reappear after that frame or after reset. Persistent dictionaries
survive both `reset` and `reset_with_options`.

Configure an encoder before processing a frame or after reset. Configure a
decoder before a standard frame header starts or after reset. Active-frame
changes return `InvalidState` without changing the selection. A finished encoder
requires reset before further configuration. See [streaming usage](streaming-usage.md)
for caller-buffer, progress, Flush/Finish and EOF contracts.

Dictionary content does not count toward output or checksum limits. Decoder
window limits still apply to advertised frame history. The retained encoder
search horizon is currently at most 128 KiB of prior output, while dictionary
content is kept separately. Prepared encoder entropy reuse and compression-level
selection remain later improvements. Context capacity accounting includes the
shared dictionary allocation once per context; summing multiple contexts may
count the same shared object more than once.

## Independent Rust training

```rust
use zstdoxide::{
    DictionaryTrainingOptions, RawTrainingOptions, train_dictionary,
    compress_with_dictionary, decompress_with_dictionary, DecodeOptions,
};

let samples: &[&[u8]] = &[
    b"GET /orders/123 HTTP/1.1\r\ncontent-type: application/json\r\n",
    b"GET /orders/456 HTTP/1.1\r\ncontent-type: application/json\r\n",
    b"GET /orders/789 HTTP/1.1\r\ncontent-type: application/json\r\n",
];
let options = DictionaryTrainingOptions {
    selection: RawTrainingOptions {
        dictionary_size: 1024,
        max_sample_bytes: 64 * 1024,
        segment_size: 64,
    },
    dictionary_id: None,
};
let dictionary = train_dictionary(samples, options)?.into_shared();
assert!(dictionary.is_formatted());
assert!(dictionary.as_bytes().len() <= 1024);
let input = b"GET /orders/321 HTTP/1.1\r\ncontent-type: application/json\r\n";
let frame = compress_with_dictionary(input, &dictionary)?;
assert_eq!(
    decompress_with_dictionary(&frame, &dictionary, DecodeOptions::new(1024))?,
    input,
);
# Ok::<(), zstdoxide::Error>(())
```

`train_raw_dictionary(samples, RawTrainingOptions)` selects useful raw content.
`train_dictionary` adds a learned full-alphabet literal Huffman table, predefined
sequence priors, an ID, and repeat offsets `[1, 4, 8]`. `finalize_dictionary`
accepts caller-selected content and samples, constructing the same metadata.
The complete formatted serialization fits `dictionary_size`, including metadata.
Finalization preserves the content suffix when trimming is necessary and adds
leading zeros if fewer than eight content bytes remain. It accepts empty samples
and builds a complete pseudocount tree. Content selection requires sufficient
sample data and otherwise returns `TrainingFailed`.

Raw dictionary capacity is at least eight bytes; formatted capacity is at least
256 bytes. Capacity cannot exceed `max_sample_bytes`, which is bounded by
`u32::MAX`. Segment sizes range from eight to 4096 bytes. Proportional prefixes
bound the total observed sample bytes, with integer rounding toward zero.
Training is deterministic and owns its result. Dynamic storage is reserved
fallibly; invalid options return `InvalidParameter`, insufficient capacity for
metadata plus eight content bytes returns `ResourceLimit`, and allocation
failure returns `AllocationFailed`.

An explicit training ID may be any nonzero `u32`. An automatic ID hashes the
original selected or supplied content before trimming or padding and lies in
`32768..2^31`. IDs are selection labels, not collision-resistant integrity
checks. Persist and distribute the same complete dictionary to encoder and
decoder.

The trainer uses independent greedy segment selection and sample-byte literal
statistics. It does not reproduce COVER/fastCover, dictionary shrink search,
compression-level-specific entropy optimization, or upstream encoded bytes.
[Training design](dictionary-training.md) records the algorithm and allocation
bounds; [upstream mappings](upstream-dictionary-plan.md) distinguish executable
assertions from the remaining algorithm and parameter work.
