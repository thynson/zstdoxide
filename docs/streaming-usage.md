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

## Selecting bounded match effort

Set `EncoderOptions::strategy` to `CompressionStrategy::Chain` to search up to
eight candidates at each four-byte key. `Greedy` is the default and searches
one candidate. All strategies share entropy coding, dictionary/window checks,
and the same 128 KiB retained-output horizon. These policies are independent
of upstream numeric compression levels.

`CompressionStrategy::Lazy` uses the same bounded chain storage and probes one
byte ahead, with sixteen candidate visits at each position. It delays only if
the next match is longer than the current match plus one byte, accounting for
the extra literal. Ties keep the current match. It can improve ambiguous
matches at the cost of additional source comparisons; it does not guarantee a
smaller frame for every input.

Chain's heads and nodes occupy 9 MiB on 64-bit targets; dictionary preparation
uses another 3 MiB seed allocation containing at most 65,536 positions.
`retained_capacity()` includes this storage, other workspaces, and the selected
shared dictionary. Reset retains allocations and the strategy. Change the
policy for a new frame using `reset_with_options`; previously allocated chain
storage remains reusable and counted even after selecting Greedy.

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
