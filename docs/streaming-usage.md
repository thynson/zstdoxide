# Using bounded streaming

`Decoder` and `Encoder` own their reusable state and borrow caller slices for
one `process` call. `Progress` gives exact consumed and produced prefixes.
Preserve unconsumed input; never advance by the length originally offered.

## Encoding a frame

This example uses Finish with a complete source slice, but the encoder still
accepts and emits bounded prefixes. Unknown-length producers normally use
Continue for source chunks, Flush when accepted bytes must be decodable, and
Finish at EOF. A pending Flush/Finish must retain its directive until complete.

```rust
use zstdoxide::{Encoder, EndDirective, StreamStatus};

let source = b"resumable, resumable, resumable";
let mut encoder = Encoder::new();
let mut position = 0;
let mut encoded = Vec::new();
loop {
    let mut destination = [0; 7];
    let progress = encoder.process(
        &source[position..], &mut destination, EndDirective::Finish,
    )?;
    position += progress.consumed;
    encoded.extend_from_slice(&destination[..progress.produced]);
    if progress.status == StreamStatus::FrameFinished { break; }
}
assert_eq!(position, source.len());
# Ok::<(), zstdoxide::Error>(())
```

The default advertised window is 128 KiB, checksum is enabled, and content size
is unknown. `EncoderOptions` can select window logs 10–31, checksum, an exact
`pledged_size`, and content-size emission. `Some(0)` pledges an empty frame;
`None` allows unknown length. Pledges are enforced even when their size is absent
from the header. Reset clears the pledge and preserves the other options.

Flush completes only when every byte originally offered to that operation has
been accepted and its encoded output delivered. An empty input retry may drain
pending output; if source remains, `NeedInput` requests its next suffix. A flush
keeps the frame open. Finish returns `FrameFinished` only after the final block
and checksum drain. A repeated empty Finish is idempotent. Start another frame
with `reset()` or validated `reset_with_options(options)`.

## Replacing settings before a frame

`set_options(options)` replaces all supported options at a fresh configuration
boundary. It preserves the selected dictionary or unclaimed prefix, buffers and
allocator owners, and does not allocate. A boundary exists before the first
`process` call or after session reset. Even a process call with empty input and
output starts the header and closes that boundary.

```rust
use zstdoxide::{CompressionStrategy, Encoder, EncoderOptions, EndDirective, Error};

let mut encoder = Encoder::new();
let options = EncoderOptions {
    strategy: CompressionStrategy::Chain,
    window_log: 16,
    checksum: false,
    ..EncoderOptions::default()
};
encoder.set_options(options)?;
assert_eq!(encoder.options(), options);

let invalid = EncoderOptions { window_log: 9, ..options };
assert_eq!(encoder.set_options(invalid), Err(Error::InvalidParameter));
assert_eq!(encoder.options(), options);

encoder.process(&[], &mut [], EndDirective::Continue)?;
assert_eq!(encoder.set_options(options), Err(Error::InvalidState));
assert_eq!(encoder.options(), options);
# Ok::<(), zstdoxide::Error>(())
```

Rejected replacement leaves the whole session unchanged, including any pending
Flush/Finish and its poison state. It does not cancel a frame. To discard a
session and install validated settings together, use `reset_with_options`.
Reference numeric levels and reference strategies belong to parameter
inspection; this method accepts only the implemented named Rust policies.

## Resetting an encoder

The reset operations have separate session and parameter effects:

| Operation | Session and counters | Settings | Dictionary and prefix selection |
| --- | --- | --- | --- |
| `reset()` | Discard the session; ready with zero counters | Preserve persistent options; clear the source pledge | Preserve a persistent dictionary or unclaimed prefix; release a claimed prefix |
| `reset_with_options(options)` | Validate first, then discard the session | Install supplied options, including their pledge | Preserve the same selection contract as `reset()` |
| `reset_parameters()` | Accept only fresh or finished; preserve counters and phase | Restore Rust defaults, including no pledge | Clear all selected and workspace attachments and cached dictionary seeds |
| `reset_all()` | Discard any session, including a failed one; ready with zero counters | Restore Rust defaults | Clear all attachments and dictionary seeds |

A completed encoder remains finished after `reset_parameters()`. Its `options()`
now reports the newly configured defaults, while its counters still describe
the completed session. Use `reset()` or `reset_all()` before another frame.

```rust
use zstdoxide::{Encoder, EncoderOptions, EndDirective, StreamStatus, decompress};

let mut encoder = Encoder::with_options(EncoderOptions {
    window_log: 10,
    checksum: false,
    pledged_size: Some(3),
    ..EncoderOptions::default()
})?;
let mut output = [0; 32];
let progress = encoder.process(b"abc", &mut output, EndDirective::Finish)?;
assert_eq!(progress.status, StreamStatus::FrameFinished);
let counters = (encoder.total_input(), encoder.total_output());
encoder.reset_parameters()?;
assert_eq!(encoder.options(), EncoderOptions::default());
assert_eq!((encoder.total_input(), encoder.total_output()), counters);
assert_eq!(encoder.process(&[], &mut [], EndDirective::Finish)?.status,
           StreamStatus::FrameFinished);

encoder.reset_all();
assert_eq!((encoder.total_input(), encoder.total_output()), (0, 0));
let progress = encoder.process(b"after", &mut output, EndDirective::Finish)?;
assert_eq!(progress.status, StreamStatus::FrameFinished);
assert_eq!(decompress(&output[..progress.produced], 5)?, b"after");
# Ok::<(), zstdoxide::Error>(())
```

Parameter/full reset retain owned codec buffers and their allocator values;
they do not allocate, clone allocators or shrink workspace. Clearing the last
shared dictionary handle can release that dictionary's separately owned
payload. Clearing a borrowed `DictionaryView` does not shorten the lifetime
required by the context type. `reset_parameters()` rejects active and failed
sessions unchanged; `reset_all()` explicitly recovers them.

These are the Rust encoder's contracts. The reference C parameter reset has
different pledge/checksum defaults and C automatically accepts a subsequent
frame. Decoder output/window limits remain governed by `DecodeOptions` and its
existing session-reset API.

## Selecting bounded match effort

Set `EncoderOptions::strategy` to `CompressionStrategy::Chain` to use bounded
candidate search. Its default examines up to eight candidates at each four-byte
key. `Greedy` is the default policy and searches one candidate. All strategies
share entropy coding, dictionary/window checks, and a configurable retained-output
horizon whose default is 128 KiB. These policies are independent of upstream
numeric compression levels.

`CompressionStrategy::Lazy` uses the same bounded chain storage and probes one
byte ahead, with sixteen candidate visits at each position by default. It delays only if
the next match is longer than the current match plus one byte, accounting for
the extra literal. Ties keep the current match. It can improve ambiguous
matches at the cost of additional source comparisons; it does not guarantee a
smaller frame for every input.

The default Chain heads and nodes occupy 9 MiB on the tested 64-bit layout;
default dictionary preparation uses another 3 MiB seed allocation containing
at most 65,536 positions. Configured dimensions change these requested sizes.
`retained_capacity()` includes this storage, other workspaces, and the selected
shared dictionary. Reset retains allocations and the strategy. Change the
policy for a new frame using `reset_with_options`; previously allocated chain
storage remains reusable and counted even after selecting Greedy.

`MatchSearchOptions` controls real hash/chain dimensions, exact key widths,
Chain/Lazy visit limits and up to 1 MiB of retained output, further bounded by
the frame window. Install it with `set_search_options` at a fresh boundary.
Session reset preserves search settings; parameter/full reset restores them.
`search_retained_capacity()` separates all retained search caches from the
wider total. See [search usage](match-search-usage.md) for ranges and examples.

## Decoding a frame

```rust
use zstdoxide::{compress, Decoder, DecodeOptions, StreamStatus};

let source = b"resumable, resumable, resumable";
let encoded = compress(source)?;
let mut decoder = Decoder::new(DecodeOptions::new(1024));
let mut position = 0;
let mut decoded = Vec::new();
loop {
    let mut destination = [0; 5];
    let progress = decoder.process(&encoded[position..], &mut destination)?;
    position += progress.consumed;
    decoded.extend_from_slice(&destination[..progress.produced]);
    if progress.status == StreamStatus::FrameFinished { break; }
    if progress.status == StreamStatus::NeedInput && position == encoded.len() {
        decoder.finish()?; // Reports truncation if the frame is incomplete.
    }
}
decoder.finish()?;
assert_eq!(decoded, source);
assert_eq!(position, encoded.len());
# Ok::<(), zstdoxide::Error>(())
```

For concatenated frames, keep processing after each `FrameFinished` until the
input producer reaches EOF. The decoder reports each standard/skippable frame
boundary separately and consumes no bytes from the following frame in that
call. `finish()` verifies a clean EOF after at least one frame and seals the
context. Reset begins a new session and zeroes cumulative counters.

Raw blocks emit before their complete payload arrives. RLE output may drain
with empty input. Compressed blocks stage at most 128 KiB of compressed data and
128 KiB of decoded data; entropy tables persist through blocks. The active ring
retains at most the frame's advertised window, capped by the remaining session
output allowance. Literal decode scratch is an additional temporary block-sized
allocation. Prior output is never retained solely because it was delivered.
`retained_capacity()` reports currently owned heap capacity, excluding inline
state, caller buffers, and temporary allocations within a call. Reset retains
primary buffer allocations; frame-local entropy tables are released.

Both codecs allow empty slices and return an explicit resource status without a
hidden retry limit. Provide the requested resource instead of repeatedly calling
with the same exhausted slices. An error poisons the context until reset. Output
from a failed call must be discarded, and earlier decoder output remains
provisional until that frame's checksum succeeds. The API does not retain caller
borrows or expose unsafe operations.
