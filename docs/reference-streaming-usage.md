# Request-backed reference streaming

`ReferenceStreamEncoder` creates incremental Zstandard frames from a
`CompressionParameterRequest`, without first constructing a dictionary.
It admits Fast, DoubleFast, Greedy, Lazy, Lazy2 and BinaryTreeLazy2 within the
existing bounded reference-plan limits. Requested levels resolve to families
and table dimensions; a valid upstream request can still exceed these bounds.
Unsupported requests return an error rather than selecting another policy.

```rust
use zstdoxide::{
    CompiledStreamOptions, CompressionParameterRequest, EndDirective,
    ReferenceStreamEncoder, StreamStatus,
};

let source = b"a small reference streaming frame";
let request = CompressionParameterRequest::with_level(3);
let mut encoder = ReferenceStreamEncoder::with_options(
    request,
    CompiledStreamOptions {
        pledged_size: Some(source.len() as u64),
        checksum: true,
        ..Default::default()
    },
)?;
let mut position = 0;
let mut frame = Vec::new();
loop {
    let mut output = [0_u8; 13];
    let progress = encoder.process(
        &source[position..], &mut output, EndDirective::Finish,
    )?;
    position += progress.consumed;
    frame.extend_from_slice(&output[..progress.produced]);
    if progress.status == StreamStatus::FrameFinished {
        break;
    }
}
assert_eq!(position, source.len());
# Ok::<(), zstdoxide::Error>(())
```

The shared frame-options type retains its existing name,
`CompiledStreamOptions`; using it does not require a compiled dictionary.
`with_options_in` and `new_in` accept a caller workspace allocator. Construction
validates the initial profile and makes no workspace allocation. Every block
parser and staging allocation uses that allocator and can fail explicitly.

## Size knowledge and reset

The request is immutable for the owner's lifetime. At the first process call
of each frame it resolves against the explicit `pledged_size`, or Unknown,
and the current prefix's matchable content length. `parameters()` returns
None before this initialization and the applied record afterward. A source
hint affects geometry only; it does not promise a byte count or create a
content-size field. An initial Finish neither infers nor overrides a pledge.
A supplied pledge is enforced even with `include_content_size: false`.

`reset()` discards the frame, clears the pledge and retains request, other
flags and workspace capacity. `reset_with_options` replaces flags and pledge;
the next process call performs admission before writing any header bytes.
A profile admitted for a tiny known source can become unsupported after a
reset to unknown size. Such a processing failure poisons the session until
reset. The engine never reports the previous frame's geometry as a new frame's
parameters.

Headers deliberately use the legal non-single-segment representation, even
for tiny known sizes. Their window descriptor tracks the resolved profile.
The pinned native encoder can instead use a single segment whose window is
exactly the source size; byte equality with native frames is not promised.

## One-frame prefixes

`with_prefix(request, options, view)` and `with_prefix_in` construct directly
from a borrowed `DictionaryView`; they build no compiled dictionary. Raw views
contribute history only. Formatted views also contribute their identifier and
initial entropy tables. Profile resolution uses matchable content length,
consistent with the existing Rust dictionary parameter APIs; a native
formatted prefix also contains its serialized header, so exact formatted-prefix
profile identity is not promised.

`set_prefix(view)` replaces a pending prefix at a clean boundary before the
header starts. An empty prefix detaches it. Initialization claims the prefix
for exactly one frame; completion or reset of that started frame releases it.
Resetting an unstarted frame keeps its pending prefix. `selection()` observes
that pending or active view. A subsequent frame resolves with dictionary size
zero unless another prefix was selected. After completion, explicitly reset
before selecting a new prefix or starting another frame.

The prefix's lifetime is borrowed from its owner. Retained history and the
prefix suffix are staged within the resolved window, and per-block parser
tables are rebuilt. Prefix buffers are never owned or mutated by the encoder.
The compiled-stream selection APIs and fixed-profile contract remain separate.

## Pumping and error contract

Use Continue for ordinary input, Flush to publish a non-final boundary, and
Finish to complete the frame. Always resubmit unconsumed input. Flush/Finish
latch the directive and original offered length until complete; subsequent
calls may provide smaller fragments of the outstanding suffix or no input to
drain pending output. Zero-length output can accept a bounded source prefix;
it cannot drain pending bytes. Output outside `progress.produced` is unchanged
on successful calls.

Any process error poisons the session. Discard output touched by that failed
call and reset before reuse. Selection boundary errors leave the active frame
intact. Empty Finish after completion is idempotent; other calls require reset.

Optimal/Ultra/Ultra2 streaming, mutable numeric parameters, parameter-only
resets, native workspace attachment policies, LDM, workers and complete
upstream migration remain open. See [design](reference-streaming-design.md)
and [validation](reference-streaming-validation.md) for the bounded claims.
