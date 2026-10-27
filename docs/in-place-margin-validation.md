# In-place margin component qualification

`decompression_margin` computes framing overhead plus the largest advertised
block maximum without allocating or decoding output. It traverses concatenated
standard and skippable frames with checked cursor/range arithmetic. Empty input
returns zero. This is a structural metadata API; success does not establish
valid block contents, truthful content sizes or correct checksums. Actual
in-place decompression is a separate, unfinished work item.

The private layout is an ordinary value record. Existing checked `ByteCursor`
access retains its independent `unsafe-byte-cursor` implementation; no new
unsafe operation or artificial unsafe value representation is introduced.

## Component and real-crate integration

The component receipt records 59 commands. Twelve tests, including eight new
scanner cases and four existing cursor/header cases, pass in every one of the
22 feature selections. Safe, cursor and all builds additionally pass true
no_std, strict component Clippy and real-file probes; safe/all Rustdoc examples
and formatting pass. Tests cover every header-field width, all 16 skip magics,
raw/RLE/compressed encoded-length traversal, multiple blocks/frames, truncation,
reserved fields, content-size extremes and arithmetic overflow without enormous
allocations. Structurally traversable invalid bodies remain explicit examples.

The separate real-crate receipt records 13 focused commands. Safe, isolated
cursor and all features each pass eight scanner unit tests, one public Rustdoc,
true no_std and strict production Clippy; formatting also passes. This verifies
the actual module/export against the qualified predecessor production. These
focused checks do not constitute a fresh whole-codec regression matrix.

## Pinned native structural comparison

Normal and ASan/UBSan helpers call the v1.5.7 `ZSTD_decompressionMargin` from
revision `f8745da6ff1ad1e7bab384bd1f9d742439278e99`. They run 10,751 descriptors
including all 96 attributed generated decodecorpus frames, every selected
truncation, synthetic header/block/skip boundaries and mutations. Safe, isolated
cursor and all Rust probes run the same descriptors. Every common successful
margin is equal; 10,734 complete success/error outcomes agree.

Seventeen descriptors are rejected by the native 64-bit non-single-segment
window-log cap of 31 while the existing Rust `FrameHeader` parser accepts the
full format field. Rust reports a structural margin capped at the 128 KiB
block maximum. This does not admit a corresponding decoding resource limit.
Native results are 2,470 successes and 8,281 errors per run; Rust results are
2,487 structural successes and 8,264 errors. The initial universal-parity
assertion failed on these cases and is preserved with its exact source,
executables and ledgers; it is not relabelled as a successful comparison.

An additional 40 descriptors cover 64-bit content-size boundaries, single and
non-single segments and checksum presence. Four native rejections correspond
exactly to advertised content size `u64::MAX - 1`, the native
`ZSTD_CONTENTSIZE_ERROR` sentinel. Rust represents that advertised value as
metadata and computes a margin. All other outcomes/margins in this supplement
agree. Native numeric error codes and valid decoded contents are not asserted
equivalent. The original and final supplemental observations are retained.

Combined, the two campaigns execute 21,582 native calls and 32,373 Rust probe
calls. No C function is linked into the Rust implementation. Downloaded
reference source stays external; helper source, reference identities, native
executables, exact inputs, fixture attribution and per-input outcomes are
retained in the component archive.

## Foreign component qualification

Four strict-provenance Miri runs execute all twelve tests under safe/all on
i686 and s390x. Two thumbv6m core+alloc checks and two concrete release LLVM
IR/ARM object builds pass. The code-generation probe actually calls the scanner;
objects are little-endian 32-bit ARM ELF. These are component interpretation
and code-generation checks, not native foreign-hardware execution or testing of
an overlapping decoder that has not yet been implemented.

Evidence: `evidence/2026-10-26-in-place-margin-component.json` and
`evidence/2026-10-26-in-place-margin-integration.json`. The remaining Module 28
items include checked overlapping storage, block orchestration, actual native
shared-buffer decoding, full regression, allocation/performance comparisons and
final reconciliation. Original in-place fuzzer groups remain partial.
