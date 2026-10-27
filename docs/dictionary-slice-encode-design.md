# Dictionary-backed caller-owned compression

Module 34 follows ordinary caller-owned compression. This design and decomposition
precede implementation; promote only after Module 33 closes. Extend the existing
Greedy one-shot encoder to persistent raw/prepared dictionary selection. Reference
levels, streaming, reusable encoder configuration and single-use prefix selection
remain separate contracts.

## Ownership and upstream counterparts

Pinned facebook/zstd v1.5.7 revision
`f8745da6ff1ad1e7bab384bd1f9d742439278e99` exposes bounded dictionary compression
through ZSTD_compress_usingDict and ZSTD_compress_usingCDict. Its exact frame bytes,
capacity-dependent choices, workspace layout and error precedence are not the
Rust implementation's contract. Existing Rust compress_with_dictionary[_in] is
the exact output oracle for this extension; external C verifies interoperability.

Reuse FrameSink, EncodedFrameHeader, the one-block staging Vec and BlockEncoder.
A borrowed DictionaryView carries immutable dictionary bytes, ID, prepared entropy
and content ranges. It owns no allocation and cannot outlive the supplied owner.
The existing sealed DictionaryHandle includes Clone, but the new output wrapper
should take a view and avoid cloning the shared handle. A copied view is plain
borrowed metadata and does not require an unsafe implementation. All existing
performance-sensitive structures retain their independent safe/unsafe features.

Generalize the internal ordinary frame emitter to an optional immutable dictionary
view. Header serialization includes the smallest legal ID field. Start a fresh
BlockEncoder with that view and the same window/input length used by the existing
allocating dictionary encoder. Dictionary seeds, literal/sequence entropy and
repeat offsets remain in their current implementations. Do not copy dictionary
history, collect a full compressed frame or retain caller borrows after return.
The existing dictionary-free wrapper must preserve its exact output and checks.

Dictionary allocation is independent of scratch allocation. The dictionary owner
must neither allocate nor change during compression; scratch clones may be
non-equivalent and each owns/frees its exact allocation domain. Borrowed non-Clone
allocators remain accepted by reference. A view avoids the Global outer allocation
of creating a shared dictionary merely for this operation.

## Proposed public contract

Add compress_into_with_dictionary(input, output, &SharedDictionary) and generic
compress_into_with_dictionary_in(input, output, &D, allocator), with the existing
sealed handle and allocator bounds. Naming and argument order follow the current
caller-owned codec APIs. A later implementation may factor an internal view-based
entrypoint without exposing additional public selection state.

Success returns the exact length and bytes of compress_with_dictionary[_in],
preserving every output byte after that length. The destination can be smaller
than a sufficient compression_bound when the actual frame fits. Short output
returns ResourceLimit and may contain earlier complete header/block appends;
allocation failure releases scratch and leaves dictionary state unchanged. No
whole-call atomicity, overlapping input/output or input/dictionary mutation is
promised. Dictionaries are caller-selected immutable objects; malformed serialized
bytes are rejected by their existing constructors, not reparsed by each encode.

The existing compression_bound already documents ordinary dictionary variants.
The same sufficient bound carries over because emitted bytes remain identical.
Its formula also bounds the direct framing worst case: a header uses at most17
bytes, checksum4, and each block adds3 bytes to at most its raw input length.
For n <128KiB, floor(n/256)+floor((128KiB-n)/2048) is at least63, exceeding the24
bytes of maximum single-block framing overhead. For n=k*128KiB+r with k>=1, the
formula margin is at least512k while framing overhead is at most3k+24. Thus the
largest ID field needs no extra output allowance. Checked numeric admission and
allocation failure remain independent; test actual dictionary fixtures and
pointer-width boundaries before publishing this variant. This proof does not
extend the bound to arbitrary streaming flushes or concatenated frames.

## Qualification

Compare exact existing Rust bytes for raw, prepared, empty-content, zero/nonzero
ID widths, prepared entropy reuse, dictionary-to-frame crossings and multiblock
history. Verify no handle clone, independent allocator ownership, dictionary byte
immutability and every observed scratch failure ordinal. Exercise exact and short
capacities, errors followed by a fresh independent call, output guards/tails and
borrow lifetimes. Include the dictionary-free predecessor's focused regressions.

Use normal and sanitized external native helpers with real dictionary and output
pointers for complete success-byte interoperability. Keep native compressed-byte
identity, random capacity selectors and original assertion provenance separate.
Retain deterministic corpus selection and explicit training differences rather
than skipping rejected cases. Qualify i686/s390x interpretation, actual no-atomics
ARM code generation, every isolated feature, all/default, no_std, strict Clippy
and release regression on a frozen source. Measure destination, one-block staging,
workspace and dictionary accounting separately, with all scratch inside timing,
caller setup outside, and independent safe/all rebuilds after CPU barriers.
