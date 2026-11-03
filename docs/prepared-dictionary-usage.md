# Compiled dictionary usage

Module 37 provides reusable immutable dictionary state for one-shot reference
profiles. A compiled state fixes one resolved profile and owns its match-finder
tables; calls do not reseed them.

## Construct once

```rust
use zstdoxide::{CompiledDictionary, CompressionParameterRequest, Dictionary};

# fn main() -> zstdoxide::Result<()> {
let dictionary = Dictionary::raw(b"shared reference dictionary content")?;
let input = b"shared reference dictionary content";
let mut request = CompressionParameterRequest::with_level(1);
// The hint makes the construction record match the later call resolution.
request.set_source_hint(u32::try_from(input.len()).unwrap())?;
let compiled =
    CompiledDictionary::new(dictionary.view(), &request, std::alloc::Global)?;
# Ok(())
# }
```

The state borrows the dictionary view for its whole lifetime and cannot outlive
the dictionary owner. Empty dictionary content returns `InvalidParameter`;
Optimal, Ultra and Ultra2 return `UnsupportedFeature` before allocating. Use
the module 36 parameterized path for empty dictionaries.

## Allocating and caller-owned calls

```rust
use zstdoxide::{
    CompiledDictionary, CompressionParameterRequest, DecodeOptions, Dictionary,
    compress_into_with_compiled, compress_with_compiled, decompress_with_dictionary_in,
};

# fn main() -> zstdoxide::Result<()> {
let dictionary = Dictionary::raw(b"shared reference dictionary content")?;
let input = b"shared reference dictionary content";
let mut request = CompressionParameterRequest::with_level(1);
request.set_source_hint(u32::try_from(input.len()).unwrap())?;
let compiled =
    CompiledDictionary::new(dictionary.view(), &request, std::alloc::Global)?;
let frame = compress_with_compiled(input, &request, &compiled)?;
let mut output = [0_u8; 128];
let written = compress_into_with_compiled(input, &mut output, &request, &compiled)?;
assert_eq!(&output[..written], &frame[..]);
assert_eq!(
    decompress_with_dictionary_in(
        &frame,
        &compiled.view(),
        DecodeOptions::new(input.len()),
        std::alloc::Global,
    )?,
    input
);
# Ok(())
# }
```

Every call re-resolves its request against the actual source length and the
dictionary content length. A different seven-field profile returns
`InvalidParameter` before output admission; the compiled tables are never used
for mismatched parameters. Caller-owned frames append complete headers and
blocks atomically and preserve bytes outside the destination slice.

## Coordinator reuse

```rust
use zstdoxide::{
    CompiledDictionary, CompiledOneShotEncoder, CompressionParameterRequest, Dictionary,
};

# fn main() -> zstdoxide::Result<()> {
let dictionary = Dictionary::raw(b"shared reference dictionary content")?;
let input = b"shared reference dictionary content";
let mut request = CompressionParameterRequest::with_level(1);
request.set_source_hint(u32::try_from(input.len()).unwrap())?;
let compiled =
    CompiledDictionary::new(dictionary.view(), &request, std::alloc::Global)?;
let encoder = CompiledOneShotEncoder::new(compiled);
let mut output = [0_u8; 128];
for _ in 0..3 {
    let written = encoder.compress_into(input, &request, &mut output)?;
    assert!(written > 0);
}
# Ok(())
# }
```

The coordinator owns the compiled state and no request, source, destination or
workspace. Repeated calls reuse the same tables and prepared entropy. Passing a
request that resolves to a different profile fails without changing the state.

## Supported profiles and limits

Fast, DoubleFast, Greedy, Lazy, Lazy2 and BinaryTreeLazy2 compile. Only the
in-window dictionary suffix contributes matches; window expiry uses the
constructed profile. Formatted dictionaries seed the first emitted block with
their prepared entropy and write their identifier. Raw dictionaries compile no
entropy. Compiled-frame bytes match the combined-staging path exactly for the
frozen Fast, DoubleFast, chain, row and formatted-seed comparisons; the eagerly
sorted tree records measured differences and is not a byte-identity contract.

Compiled tables are read-only during calls. Per-call sampling, scratch buffers
and output remain in the call's allocator domain; the compiled state's allocator
domain is independent and survives until the state is dropped. Native CCtx
workspace reuse, LDM, workers, streaming and CLI parity are outside this
bounded API.
