# Dictionary-backed in-place decompression

`decompress_in_place_with_dictionary(buffer, input_start, &dictionary, options)`
borrows a shared raw/prepared dictionary while decoding the buffer's encoded
suffix into its prefix. `decompress_in_place_with_dictionary_in` additionally
accepts a shared owner or borrowed `DictionaryView` through `DictionaryHandle`
and a separately typed scratch allocator. Dictionary ownership is not cloned;
a borrowed non-Clone allocator can supply scratch as `&allocator`.

Compute a sufficient layout using the encoded stream's `decompression_margin`,
then allocate at least the maximum of encoded length and expected decoded length
plus margin. Place encoded bytes at the end. Dictionary bytes occupy independent
immutable storage and do not count against output capacity. Smaller layouts may
work, including raw overlap, but dynamic fences always protect unread input.

One selection applies to every standard frame in the call. Skippable frames keep
it. Nonzero dictionary IDs must match; omitted IDs still use the selection. Each
frame resets output history eligibility, repeat offsets and borrowed entropy
fallback independently. Frame-local entropy overrides and literal scratch use the
scratch allocator; they never change dictionary content or adopt its allocator.
There is no complete input/output clone or copied dictionary/history window.

The effective output allowance is `min(buffer.len(), options.max_output_size)`.
On success, the returned prefix is valid and every later byte remains unchanged.
On error, writes within that allowance can be provisional; bytes beyond it and
unread encoded input remain protected. Invalid input indices are rejected before
writes. Content sizes/checksums are verified. The advertised-window limit remains
independent. Dictionary mismatch precedes frame resource checks; native numeric
errors and error precedence are not promised.

The existing dictionary match policy permits initial distances beyond the window
while frame position has not passed that window. It rejects dictionary references
after frame position exceeds the window; native one-shot decoding accepts the
retained p-window-plus-one fixture. This difference is explicitly tested. Empty
input returns UnexpectedEof, unlike native empty concatenation success. Valid
empty/skippable frames can return zero. Raw/RLE/skippable paths add no codec payload
allocation merely because a dictionary is selected.

A DictionaryView borrows a validated owner and cannot outlive it. The view exposes
immutable content, so safe Rust cannot pass that content as the exclusive mutable
destination simultaneously. The call does not retain the output or input borrow.
Single-use in-place prefixes, incremental overlap, external workspaces, legacy and
magicless frames remain outside this API. Validation and performance have their
own receipts; this is not complete upstream libzstd or CLI parity.
