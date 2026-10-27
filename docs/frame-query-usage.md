# Checked frame queries

`frame_info(input)` returns copied FrameInfo for the first complete standard or
skippable frame. `kind()`, `encoded_size()`, `block_count()` and `content_size()`
expose its parsed properties. `frame_compressed_size(input)` returns that first
encoded extent. Both ignore trailing input, allowing callers to advance slices.

`decompressed_size(input)` scans the exact whole concatenation: Some(total) means
every standard frame advertises a size; None means at least one omits it. Empty
input and skippable-only input have size zero. Malformed/truncated trailing input
is rejected even after an unknown-size frame. `decompression_bound(input)` uses
known advertised sizes or the checked block-count/window upper bound for unknown
frames. Values use u64, including on 32-bit targets; aggregate overflow is an
error. These queries do not reserve native sentinel values.

`is_skippable_frame(input)` recognizes only the four-byte magic. It does not prove
that the rest exists. `skippable_payload(input)` validates the first complete
skippable frame and returns `(variant, borrowed_payload)` without copying input.
The borrow cannot outlive its input. Trailing input is left for the next frame.

These structural queries allocate no codec payload storage and do not decode
entropy, verify checksum values or prove an advertised size truthful. A successful
scan does not guarantee decompression succeeds. Bounds are meaningful for valid
supported frames; callers should enforce their own resource policy before using
untrusted advertised sizes. Legacy and magicless input remain unsupported.

The existing `decompression_margin` retains its separate suffix-input layout
contract; a size bound is not an in-place safety margin. See
`frame-query-validation.md` for native differences and qualification.
