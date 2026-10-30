# Parameterized dictionary one-shot compression usage

Module 36 combines the bounded reference parameter profiles with immutable
dictionary attachments in one-shot, caller-owned compression. It does not
enable streaming, live context reset, long-distance matching, workers or
arbitrary frame settings.

## Allocating dictionary frames

`compress_with_parameters_and_dictionary` resolves the request against the
actual source length and the dictionary's matchable content length, then runs
the admitted reference profile:

```rust
use zstdoxide::{
    CompressionParameterRequest, DecodeOptions, Dictionary,
    compress_with_parameters_and_dictionary, decompress_with_dictionary,
};

# fn main() -> zstdoxide::Result<()> {
let dictionary = Dictionary::raw(b"shared reference dictionary content")?.into_shared();
let input = b"shared reference dictionary content";
let frame = compress_with_parameters_and_dictionary(
    input,
    &CompressionParameterRequest::with_level(4),
    &dictionary,
)?;
assert_eq!(
    decompress_with_dictionary(&frame, &dictionary, DecodeOptions::new(input.len()))?,
    input
);
# Ok(())
# }
```

Use `compress_with_parameters_and_dictionary_in` to own output and scratch in a
specific allocator. The returned vector borrows neither the source nor the
dictionary, and every scratch owner is released before return.

## Caller-owned output

`compress_into_with_parameters_and_dictionary` writes one frame into an
existing slice and returns the exact written length. Header and block appends
are atomic; a short destination returns `Error::ResourceLimit` after earlier
whole appends and never changes bytes outside the slice.

```rust
use zstdoxide::{
    CompressionParameterRequest, Dictionary, compress_into_with_parameters_and_dictionary,
};

# fn main() -> zstdoxide::Result<()> {
let dictionary = Dictionary::raw(b"shared reference dictionary content")?;
let input = b"shared reference dictionary content";
let mut output = [0xa5_u8; 128];
let written = compress_into_with_parameters_and_dictionary(
    input,
    &mut output,
    &CompressionParameterRequest::with_level(2),
    &dictionary.view(),
)?;
assert!(written <= output.len());
assert_eq!(output[written..], [0xa5; 128][written..]);
# Ok(())
# }
```

`compress_into_with_parameters` is the dictionary-free caller-owned variant.

## Coordinator selection

`OneShotEncoder` stores dictionary selection only. A per-call request is never
retained:

```rust
use zstdoxide::{CompressionParameterRequest, Dictionary, OneShotEncoder};

# fn main() -> zstdoxide::Result<()> {
let dictionary = Dictionary::raw(b"shared reference dictionary content")?;
let input = b"shared reference dictionary content";
let mut encoder = OneShotEncoder::with_dictionary(dictionary.into_shared());
let mut output = [0_u8; 128];
let size = encoder.compress_into_with_parameters(
    input,
    &CompressionParameterRequest::with_level(4),
    &mut output,
)?;
assert!(size > 0);
# Ok(())
# }
```

A pending prefix is consumed by every attempted call before request resolution
or allocator work, including unsupported-profile and short-output errors.
Persistent selection survives attempts until replaced or cleared. When no
selection is present the method emits the same bytes as
`compress_with_parameters`.

## Supported dictionary profiles

Fast, DoubleFast, Greedy, Lazy, Lazy2 and BinaryTreeLazy2 use the combined
staging path. Optimal, Ultra and Ultra2 - fresh or retained - return
`Error::UnsupportedFeature` for nonzero dictionary content rather than dropping
the dictionary. Raw and empty dictionaries preserve the dictionary-free
parameterized bytes.

Only dictionary content that fits the resolved window can provide matches.
A formatted dictionary's prepared entropy additionally seeds the first block,
matching `ZSTD_loadCEntropy`; the identifier is written in the frame header.
Decode with the same dictionary through `decompress_with_dictionary` or
`DictionaryView`-based decoder options.

## Limits

These are bounded one-shot contracts. Native workspace reuse, CDict creation
costs, parameter lifecycle, error precedence, long-distance matching, workers,
streaming and complete upstream parity are not promised.
