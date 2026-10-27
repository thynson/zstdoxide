# One-shot in-place decompression

`decompress_in_place` receives one mutable byte slice and an index. Encoded input
occupies the suffix starting at that index; decoded output starts at zero. The
slice is the sole owner of both regions throughout the call, so no independent
input/output Rust references alias it. The caller chooses the input end by
sub-slicing its allocation before the call.

```rust
use zstdoxide::{compress, decompression_margin, decompress_in_place};

let original = b"an overlapping buffer, an overlapping buffer";
let encoded = compress(original)?;
let capacity = (original.len() + decompression_margin(&encoded)?).max(encoded.len());
let mut buffer = vec![0xa5; capacity];
let input_start = capacity - encoded.len();
buffer[input_start..].copy_from_slice(&encoded);
let written = decompress_in_place(&mut buffer, input_start)?;
assert_eq!(&buffer[..written], original);
# Ok::<(), zstdoxide::Error>(())
```

For supported valid frames, decoded size plus the structural margin is sufficient
when the input fits. The scanner does not validate decoded content, checksums or
truthfulness of the advertised size. Unknown content sizes work with caller-chosen
capacity and dynamic bounds; the API does not guess or allocate an output size.
Smaller layouts can work. Raw blocks permit real overlap; compressed and RLE
blocks protect the current payload start. These checks always apply even when a
caller calculated a sufficient margin.

`decompress_in_place_with_options` accepts `DecodeOptions`. The effective combined
output limit is `min(options.max_output_size, buffer.len())`; advertised-window
limits are independent. Concatenated frames share the output allowance but never
inherit previous-frame match history. Standard and skippable frames are supported,
and content checksums are always verified. An invalid initial index returns
`ResourceLimit` before mutation. Empty input returns `UnexpectedEof`; an empty
frame or an all-skippable stream can return zero.

Success returns the decoded prefix length and preserves every byte after it,
including remaining encoded bytes outside the output. Failure can leave
provisional writes inside the output allowance. It preserves unread encoded bytes
and bytes beyond that allowance, but does not promise rollback or error progress.
Capacity exhaustion and a collision with an unread block both return
`ResourceLimit`. Error kinds and precedence are not native numeric error codes.

`decompress_in_place_in` and `decompress_in_place_with_options_in` select a scratch
allocator. A reference to a non-Clone allocator is accepted. The output allocation
is never adopted, resized or freed. Fallible literal and entropy scratch uses
clones of the selected allocator and is released on success or error; cloning
retains the allocator's own allocation and panic semantics. Raw, RLE and skippable
blocks require no codec payload allocations. There is no complete-input clone,
complete-output allocation, or copied history in the decoder.

Safe builds use checked `copy_within` for raw overlap. The independent
`unsafe-in-place-buffer` feature uses checked `ptr::copy` with the same bounds and
error contract. Compressed match copying separately selects
`unsafe-slice-history`; both are optional. The default build remains safe Rust.

Dictionary/prefix in-place decoding, incremental overlap, stable-input sessions,
external scratch workspaces, legacy and magicless frames are outside this API.
This bounded contract does not complete the upstream migration.
