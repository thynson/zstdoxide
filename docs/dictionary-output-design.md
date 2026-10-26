# Dictionary-backed caller-owned decompression output

Module 25 is designed before implementation. Develop its candidate separately
while predecessors finish qualification, and promote it only after Module 24
closes. This design does not claim implementation or admission.

Expose two one-shot APIs consistent with the current dictionary collectors:
`decompress_into_with_dictionary(input, output, dictionary, options)` and
`decompress_into_with_dictionary_in(input, output, dictionary, options, allocator)`.
The convenience dictionary is the existing shared Global owner; `_in` accepts
`D: DictionaryHandle` by reference, including a borrowed allocator-erased view.
Only scratch belongs to `A: Allocator + Clone`. The destination remains exclusively
borrowed only for the call, with the same clamped allowance, unchanged suffix,
provisional error writes, checksum and empty-input contracts as Module 23.
Stable dictionary sessions and single-use prefixes remain a later scope.

## Reuse decision

Extend the existing generic one-shot frame helper with an optional borrowed
`DictionaryView`. The no-dictionary wrappers continue to pass `None`; preserve
their clone ordering, header/limit errors and allocation behavior. Header IDs
must distinguish missing dictionaries from mismatches, while ID zero still
allows raw/formatted dictionary use. Per frame, seed repeat offsets from
prepared entropy and pass borrowed initial Huffman/FSE views through existing
literal and sequence APIs. Reset new tables between concatenated frames.
Dictionary content is never counted as output or copied into output history.

A private borrowed adapter can implement `FrameOutput`/`SequenceOutput` over a
`SliceHistory` and dictionary view. It is a shared safe reference/value record,
not a new storage family. It forwards actual match access to SliceHistory's
existing safe / unsafe-slice-history implementations and DictionaryView's
safe / unsafe-dictionary byte access. No additional unsafe feature is needed
for plain orchestration or non-owning records.

## Critical match boundary

The existing `stream_decode.rs::BlockSink::copy_match` is the behavioral
predecessor. Let p be current-frame output length, d the nonzero distance and
W the advertised window:

- d <= p: ordinary frame match, requiring d <= W.
- d > p: dictionary match is permitted only while p <= W. The initial distance
  may exceed W. Let k = d - p. Require k <= dictionary.content().len(), so the
  dictionary start is content.len() - k.
- For a dictionary match, the first min(length,k) bytes read dictionary content;
  remaining bytes read frame positions starting at zero, including bytes just
  produced by the same forward-expanding match. Validate the initial match once.
  Do not delegate the remainder to ordinary `copy_match(d, remaining)` when
  d > W: that would incorrectly reject a valid dictionary-to-frame transition.
- The repository deliberately rejects dictionary references starting after
  p > W even where the pinned C implementation accepts them; retain and cite
  the established format-based qualification in `docs/dictionary-design.md`.

Extend checked SliceHistory with one dictionary-aware operation rather than
exposing mutable storage. Validate distance/dictionary range and the whole
output end before any write. Prove source < destination for the frame part:
its constant difference is d > 0, including newly produced bytes. The entire
slice is initialized, bounds/lifetime derive from its exclusive borrow, and
u8 alignment is one. The unsafe variant may use the output pointer only after
these checks; dictionary reads still go through checked DictionaryView access.
Extreme distances/lengths, zero-length matches, capacity errors and dictionary
expiry require independent no-mutation-on-rejection tests.

## Qualification decomposition

1. Central ownership and exact API/error/clone contracts before implementation.
2. Dictionary-aware slice match wrapper, safe/independent unsafe combinations,
   all small crossing offsets, window boundary and rejection-before-write tests.
3. Optional prepared entropy/ID orchestration without changing no-dictionary
   outputs, errors or allocator request ordering.
4. Public one-shot destination wrappers and borrowed/nonclone dictionary and
   scratch allocator lifetimes, failures, suffixes and malformed inputs.
5. Pinned native raw/prepared dictionary destinations and all existing independent
   dictionary fixture boundaries, with explicit native-format differences.
6. Matching dictionary fuzz branch/corpus analysis and truthful bounded adapters.
7. Safe/isolated/all/full/release/no_std/Clippy, foreign Miri and concrete ARM.
8. Identical direct/owned workloads, allocator ledgers, retained executables and
   independent repeat timings after correctness/CPU barriers.
9. Reconcile scope; decompose stable dictionary sessions/prefix ownership next.

## Exact orchestration and public bounds

The shared helper receives `Option<DictionaryView>` as an immutable value.
For a nonzero frame ID, no view returns `DictionaryRequired` and a mismatched
view returns `DictionaryMismatch` before window/content/output checks. ID zero
uses the selected view as configured. At each standard frame, seed repeat
offsets from its prepared entropy or `[1,4,8]`; owned Huffman and FSE state is
fresh per frame, while immutable dictionary tables remain borrowed fallback.
Skippable frames neither modify nor consume the persistent dictionary.

No-dictionary wrappers pass `None` and retain their early empty-input rejection,
allocator cloning order and allocation request sequence. Dictionary wrappers
reject empty input before any codec scratch allocation, clamp output allowance
to the destination, and borrow the selected dictionary without cloning its
shared handle or allocator. The frame helper uses existing dictionary-view
literal/sequence entrypoints with `None` for old callers. No format parser or
checksum implementation is duplicated. Existing owned dictionary collection
remains based on its buffered decoder in this milestone.

`DictionarySliceOutput` is a private record containing `SliceHistory` and a
`DictionaryView`. `FrameOutput` delegates start/frame bytes/repeat operations;
`SequenceOutput` delegates append/length and invokes the dictionary-aware match
wrapper. It owns no allocation and cannot outlive either output or dictionary.
No dictionary contents are copied into a history allocation. Success preserves
the entire unused destination suffix. Failure preserves bytes beyond the
effective allowance and may leave provisional bytes inside it. Limits, error
precedence between independent malformed fields and allocator request counts
need not equal the buffered collector; verify valid output equivalence and the
existing explicit dictionary-window qualification instead.

## Upstream fuzz scope

The pinned `dictionary_decompress.c` selects the source prefix, 100 training
sample ranges, prepared/loaded/prefix attachment mode and destination capacity.
The existing adapter preserves these selectors but routes through buffered
output. Extend the persistent prepared/loaded branches to exercise an actual
caller-owned destination with guards and the same capacity; keep prefix-only
branches on their existing API. Record branch counts so prefix executions are
not misrepresented as new direct-output coverage. Preserve the exact corpus
identity and all existing training/fallback qualifications. The independent
Rust trainer is not fastCover, loaded by-reference modes use Rust ownership,
and unsupported external sequence producer/stateful branches remain partial.
Native raw/prepared destination helpers independently verify actual fixed
destinations on valid fixtures; do not infer native equivalence from sharing
Rust dictionary training.
