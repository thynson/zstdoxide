# Stable borrowed decompression output

`SliceDecoder` holds one mutable output slice for an entire incremental session.
It uses the written bytes of the current frame as match history and writes raw,
RLE and compressed blocks directly into that slice. Input fragments are temporary
borrows. Compressed input fragments, literals and entropy can require fallible
scratch allocation; the decoder does not allocate an output vector or a ring
copy of history.

```rust
use zstdoxide::{compress, DecodeOptions, SliceDecoder};

let frame = compress(b"hello")?;
let mut storage = [0xa5; 16];
let mut decoder = SliceDecoder::new(&mut storage, DecodeOptions::new(5));
for fragment in frame.chunks(3) {
    let progress = decoder.process(fragment)?;
    assert_eq!(progress.consumed, fragment.len());
}
decoder.finish()?;
assert_eq!(decoder.output()?, b"hello");
drop(decoder);
assert_eq!(&storage[5..], &[0xa5; 11]);
# Ok::<(), zstdoxide::Error>(())
```

The effective output allowance is the smaller of slice length and
`DecodeOptions::max_output_size`. The advertised-window limit remains
independent. Insufficient fixed output returns `ResourceLimit`; this API never
requests `NeedOutput`. Drop the session before accessing or resizing its
original destination. A larger destination requires a new session and replay
of input; a failed session cannot continue by replacing its output pointer.

## Fragment and frame boundaries

A successful call returns `NeedInput` or `FrameFinished`. The latter stops
before the next frame's magic, even if the supplied fragment has more input.
Always resubmit the unconsumed suffix. Concatenated standard and skippable
frames share the combined output budget, but each standard frame gets a fresh
history boundary. Skips contribute no output.

```rust
use zstdoxide::{compress, DecodeOptions, SliceDecoder};

let first = compress(b"first")?;
let second = compress(b"second")?;
let input = [first.as_slice(), second.as_slice()].concat();
let mut storage = [0_u8; 11];
let mut decoder = SliceDecoder::new(&mut storage, DecodeOptions::new(11));
for fragment in input.chunks(7) {
    let mut used = 0;
    while used < fragment.len() {
        let progress = decoder.process(&fragment[used..])?;
        assert!(progress.consumed > 0 || progress.produced > 0);
        used += progress.consumed;
    }
}
decoder.finish()?;
assert_eq!(decoder.output()?, b"firstsecond");
# Ok::<(), zstdoxide::Error>(())
```

Empty input can complete a pending zero-sized block; otherwise it requests more
input. `finish()` validates a clean boundary after at least one complete frame.
An empty or partial stream returns `UnexpectedEof`. Successful finish can be
repeated, but a later process call requires reset.

## Output, errors and reset

`output()` borrows the written prefix, including completed earlier frames and
provisional bytes from the current frame. Those current bytes become validated
only after content size and checksum checks pass. Any error poisons the session,
denies output access and requires reset. Bytes inside the allowance can have
changed; bytes beyond it stay untouched. Dropping the failed decoder does not
make its provisional bytes a valid decoded result.

`total_input()` and `total_output()` include progress in a failing call. An error
returns no resumable slice positions. `reset()` clears counters and history on
the same borrowed destination and options, without erasing underlying bytes.
It clears encoded-input length while retaining that buffer's capacity, releases
Huffman/FSE overrides and retains the sequence allocator. Replaying input after
reset overwrites output from position zero.

`new_in` accepts an allocator or a reference to a non-Clone allocator owner for
scratch. Ordinary allocator cloning keeps that allocator's own behavior.
`retained_capacity()` reports scratch capacity and excludes inline state and
caller output. No output allocation is adopted or freed by the decoder.

The Module 24 checkpoint covers dictionary-free sessions. Dictionary-backed
sessions have their own constructor/ownership qualification. Stable input,
overlapping in-place output and arbitrary C workspace controls remain separate
contracts. See [qualification](stable-output-validation.md) and
[measurements](stable-output-benchmarks.md) for the tested boundaries.


## Persistent dictionaries with a fixed destination

`SliceDecoder::with_dictionary` consumes a persistent shared dictionary handle;
`with_dictionary_in` accepts a shared owner or borrowed `DictionaryView` plus an
independent scratch allocator. The selected dictionary supplies initial entropy
and repeat offsets for every standard frame, including frames with an omitted
ID. Nonzero IDs must match. Skips do not consume the selection, and reset retains
it. The constructor does not clone the supplied handle.

```rust
use zstdoxide::{compress_with_dictionary, DecodeOptions, Dictionary, SliceDecoder};

let dictionary = Dictionary::raw(b"hello dictionary")?.into_shared();
let frame = compress_with_dictionary(b"hello dictionary hello", &dictionary)?;
let mut storage = [0xa5; 32];
let mut decoder = SliceDecoder::with_dictionary(
    &mut storage, DecodeOptions::new(22), dictionary,
);
for fragment in frame.chunks(3) {
    assert_eq!(decoder.process(fragment)?.consumed, fragment.len());
}
decoder.finish()?;
assert_eq!(decoder.output()?, b"hello dictionary hello");
# Ok::<(), zstdoxide::Error>(())
```

A borrowed view cannot outlive its dictionary owner. Consuming a shared handle
keeps that owner alive until the session drops. Prepared table payloads are
borrowed during block calls; working overrides remain frame-local. Dictionary
and scratch allocators need not have the same type or identity.

`dictionary_retained_capacity()` reports the dictionary payload reachable from
the selection, zero when absent. It is separate from scratch-only
`retained_capacity()` and excludes shared-control storage. It does not imply
that a shared or borrowed payload is uniquely owned by this session.

Prefixes and attachment replacement are outside the Module 26 API. The bounded
persistent constructor does not expose arbitrary C context reset/parameter
operations. See [dictionary session qualification](stable-dictionary-output-validation.md)
and [measurements](stable-dictionary-output-benchmarks.md).
