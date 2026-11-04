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

## Streaming reuse

```rust
use zstdoxide::{
    CompiledDictionary, CompiledStreamEncoder, CompressionParameterRequest, DecodeOptions,
    Dictionary, EndDirective, StreamStatus, decompress_with_dictionary_in,
};

# fn main() -> zstdoxide::Result<()> {
let dictionary = Dictionary::raw(b"shared reference dictionary content")?;
let input = b"shared reference dictionary content";
let mut request = CompressionParameterRequest::with_level(1);
request.set_source_hint(u32::try_from(input.len()).unwrap())?;
let compiled =
    CompiledDictionary::new(dictionary.view(), &request, std::alloc::Global)?;
let mut encoder = CompiledStreamEncoder::new(compiled)?;
let mut frame = Vec::new();
let mut output = [0_u8; 32];
let mut progress = encoder.process(input, &mut output, EndDirective::Finish)?;
frame.extend_from_slice(&output[..progress.produced]);
while progress.status != StreamStatus::FrameFinished {
    progress = encoder.process(&[], &mut output, EndDirective::Finish)?;
    frame.extend_from_slice(&output[..progress.produced]);
}
assert_eq!(
    decompress_with_dictionary_in(
        &frame,
        &dictionary.view(),
        DecodeOptions::new(input.len()),
        std::alloc::Global,
    )?,
    input
);
# Ok(())
# }
```

`process` follows the same `Continue`/`Flush`/`Finish` resubmission rules as
`Encoder`: resubmit the unconsumed suffix, retry with more output space after
`NeedOutput`, and keep the same directive while a flush or finish is latched.
The compiled window and seven-field profile are fixed; frame options select only
the checksum, an optional pledge and the content-size/dictionary-ID flags.
`reset` keeps the compiled selection and clears the one-frame pledge;
`reset_with_options` also replaces the frame flags. The next frame installs the
compiled first-block entropy seed again. A processing error poisons the stream
until reset.

The selection itself follows the upstream reference lifecycle. `set_prefix`
replaces the compiled state with a borrowed one-frame prefix and returns the
displaced compiled value intact; the next frame initialization claims that
prefix exactly once, the frame header carries a formatted prefix's identifier
and its prepared entropy seeds the first block, and every later frame runs
dictionary-free until a new selection is chosen. `set_compiled` restores a
compiled selection whose parameters equal the construction profile and clears
a pending prefix. Only a clean boundary (before the first `process` of a
frame) accepts selection changes; `selection()` reports the active view.
Empty prefixes detach the selection like the upstream empty
`ZSTD_CCtx_refPrefix_advanced` call.

`reset_parameters` corresponds to `ZSTD_reset_parameters`: at a clean boundary
or after a completed frame it restores default frame options, removes the
selection and returns the compiled value for reuse; an active, finishing or
failed session is refused with `Error::InvalidState` without mutation, and no
table is ever reseeded. `reset_all` is the `ZSTD_reset_session_and_parameters`
form: every phase — including a poisoned one — recovers to a fresh session
with default options and no selection. Frames after a removal continue
dictionary-free under the fixed profile.

Block history is retained contiguously for at most the resolved window, so
cross-block matches and repeat offsets keep their wire distances and compiled
candidates become unreachable exactly when the decoder's window would reject
them. Compiled tables are never reseeded, copied or mutated per block. The
per-block parser owners are released when the block seals; only history and
entropy cross a block boundary. This is a bounded Rust streaming contract, not
native CCtx workspace reuse.

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
domain is independent and survives until the state is dropped. The streaming
coordinator uses its own workspace allocator and rejects a compiled window
outside the supported reference frame range. Native CCtx workspace reuse, LDM,
workers, prefix replacement, parameter resets that clear a compiled selection
and CLI parity are outside this bounded API.
