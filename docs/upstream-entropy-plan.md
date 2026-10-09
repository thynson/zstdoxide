# Module 8 upstream entropy coverage

This plan separates executable evidence from the remaining test migration.
The reference is facebook/zstd v1.5.7 revision
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`. It does not redefine the production
design in `entropy-encoder-design.md` or close the complete-migration gate.

## Test data and ownership

The current tests borrow immutable fixture descriptions and sources, own their
encoded buffers in `Vec<u8>`, and inspect wire positions with checked indices.
The standalone C oracle owns bounded input/output buffers in an external
temporary process. It is never linked into the Rust library. Future selector
records contain scalar booleans, depths, capacities and source ranges; they
need no artificial unsafe backend. The existing checked `DataProducer`
preserves the original tail-consumption order without non-owning pointers.

Decompose new work before implementation: exact selector vectors; the bounded
supported roundtrip driver; missing standalone depth/capacity contracts; a
separately pinned complete corpus replay; original dictionary/sequence
schedules; and implemented strategy adapters. Each is a separate reviewable
checkpoint. No new test implementation is included in this mapping checkpoint.

## Exact Huffman target boundary

The upstream target is
[`tests/fuzz/huf_round_trip.c`](https://github.com/facebook/zstd/blob/f8745da6ff1ad1e7bab384bd1f9d742439278e99/tests/fuzz/huf_round_trip.c).
Its single inventory case remains partial.

| Source lines | Original contract | Current evidence or remaining work |
| --- | --- | --- |
| 27–38, 68–80 | Histogram, highest nonzero symbol, and adjusted requested table depth | Sparse Rust histograms and valid descriptions exist. The exact target adjustment is not yet driven: minimum depth is `ceil(log2(max_symbol + 1)) + 1`, using highest symbol rather than cardinality. |
| 43–57 | Tail selectors, in exact order | Not yet ported for this target. Reuse the independently verified producer; preserve both BMI-dependent byte-consumption layouts. |
| 55, 58–61 | Capacity in `0..=4*original_input_len`, remaining source capped at 256 KiB | Capacity uses the original length before selector consumption/capping. Owned Rust output does not exercise the caller's destination. Production four-stream encoding currently limits sources to 128 KiB. |
| 63–77 | Empty/single-byte and uniform-source early exits | Existing encoder boundary tests are useful, but the exact target's selector-dependent early exits remain part of the new driver. |
| 90–93 | Heuristic or optimal requested-depth selection, then successful table construction with log at most twelve | The inline builder fixes its maximum to eleven. Encoding an already prepared twelve-bit dictionary tree is verified; constructing standalone requested-depth trees through twelve is a separate missing contract. |
| 94–99 | Write the table into the selected destination; accept a reported failure | Sparse descriptions are independently accepted by C. Random insufficient destination and unrepresentable-description return paths are not exercised by the owned-output builder. |
| 100–108 | Read the written table through X1 or X2; explicitly fall back on the specific X2 table-log error | The same-description C oracle performs both reads and explicit fallback. Current cases required no fallback. Rust uses one checked table representation. |
| 113–127 | Encode 1X/4X; when nonzero, decode exact source length and bytes | Thirty stored tables reproduce both C streams byte for byte. Live C checks cover successful streams and boundaries. Exact zero-result/capacity paths and the full parameter driver remain incomplete. |

The selector order is streams, decoder variant, optional BMI2 bit, optimal
depth, prefer-repeat, suspect-uncompressible, disable-assembly, disable-fast,
capacity, and requested depth. The BMI2 selector consumes a byte only when the
C host reports BMI2. No prefix-reservation selector precedes this sequence.
The remaining source begins at the original input start, not at the consumed
tail. A future portable driver should execute both consumption layouts and
compare them with independent C-produced parameter vectors.

`optimalDepth` changes `HUF_optimalTableLog` in the original target and needs
a meaningful equivalent test policy before its branch can be called complete.
`preferRepeat` and `suspectUncompressible` are still consumed, but their policy
branches are in `HUF_compress_internal`, which this target never calls. They
must not be advertised as missing active controls for this particular target.
BMI2, X1/X2, disable-assembly and disable-fast choose C implementations or
table layouts. Independent C comparisons can exercise those selectors without
inventing Rust knobs for engines the Rust implementation does not have.

The original 1X compressor returns zero for a destination below eight bytes;
4X returns zero below seventeen bytes or below twelve source bytes. These
are C compressor contracts, not wire-format minimum sizes. Rust's successful
six- and nine-byte 4X cases have an empty final segment and a marker; the C
oracle verifies them by assembling four independent 1X quarters. Do not call
that a successful invocation of the original C 4X branch. Similarly, the empty
1X `01` marker is checked without claiming a successful C zero-output decode.

Extending a test-only roundtrip by truncating every source to 128 KiB would
silently discard a real upstream contract. The target admits 256 KiB sources.
Some larger 4X streams also fail the sixteen-bit quarter-size limit. Test
the corresponding accepted or explicit failed result; do not split one
standalone stream into independent frame blocks and call it the same input.

## Executed evidence

`tests/upstream/huffman_encode.rs` has thirty exact 1X and thirty exact 4X
fixture comparisons. Its three explicit C tests passed with safe and
all-feature builds: 85 same-table invocations, 55 sparse-builder invocations,
and 59 Rust-4X invocations. The counts overlap in source tables and are not
independent upstream cases. The maximum prepared-log-12 4X case encodes a
128 KiB source into 196,618 bytes with three 49,153-byte jump entries.

`tests/entropy_encoding.rs` has eight normal public tests plus two independently
exercised CLI tests. The ordinary suite passed safe, codebook-unsafe alone,
all-feature and no-default builds. Each safe/all-feature CLI run decoded six
zero-sequence boundary frames and six frame-lifetime/dictionary frames.
These include meaningful inline/treeless mode inspection and fragmented
output. They do not establish any particular upstream numeric strategy.

Inverse FSE construction, repeat planning and supplied-table sequence encoding
are separately checked foundations. `src/sequence_encode.rs` covers all 27
predefined/RLE/new channel combinations followed by repeat; independent C
decodes their 27 frames and 54 sections. It also exercises literal length
65,536, zero-literal repeat changes, duplicate repeats, maximum logs and offset
code 31, unavailable symbols and nonzero-log dictionary singleton tables.
No corresponding upstream whole-source status is upgraded just because these
internal data structures now exist.

The normalized-count fuzz target remains completely ported. The exact
`fuzzer.c:4172` arithmetic regression remains ported; `:4190` remains partial
because C rejects its nine-byte destination despite emitting a nine-byte
header when given more destination slack. Existing Module 7 evidence records
remain historical snapshots and are not rewritten as current suite counts.

## Next original campaigns and strategy obligations

1. Preserve every original Huffman selector and remaining source range with
   independent C vectors, both BMI layouts, empty/RLE inputs and every requested
   depth. Add the supported roundtrip driver with explicit unsupported-branch
   accounting; a full external run alone cannot erase those qualifications.
2. Design standalone bounded entropy output and depth control independently of
   frame limits. Verify exact-fit/too-small destination writes and failure
   bounds before mapping the original capacity branches. Keep twelve-bit
   prepared tables distinct from new inline tables limited to eleven bits.
3. Run the complete independently pinned `huf_round_trip` corpus only after
   the matching driver exists, with no files skipped and both safe/unsafe
   builds. Preserve capacity, source-size and depth outcomes in the result.
4. Reproduce `zstreamtest.c:1831` exactly: a 128 KiB RDG(0,0,seed) block, a
   128 KiB-minus-256 RDG(.05,0,seed) region, then two 128-byte dictionary
   regions at offsets 256 and 128. Preserve prepared-dictionary setup and
   exact output comparison; the original level/by-reference setup remains
   qualified until equivalent APIs exist.
5. Reproduce `zstreamtest.c:1884`: 43,691 flushes of six `aa` bytes, then nine
   128-byte dictionary regions from offset 1,024 down to zero. Verify consumed
   source, finalization and independent decoding. The current synthetic
   raw/RLE Huffman lifetime witness is not this offset-table campaign.
6. Reproduce the complete sequence-generator sweep at `zstreamtest.c:1551`,
   including all values below 300, powers and quarter-offset values through
   `2^17`, and offsets through window log 24. Local inverse-table coverage
   and hand-written supplied-table sequences do not replace this campaign.
7. Pair each implemented named strategy with original input/assertion ports.
   `fuzzer.c:1398` also requires level 19, minimum match seven and post-sequence
   splitting; `:3265` uses the level-19 optimal parser with repeat offsets
   10/10/10. Block-splitter, superblock, multithread and numeric-level rows
   remain blocked/partial until their actual contracts are implemented.

The byte fixture at `zstreamtest.c:1743` is already a complete decoder port.
Its comment describes three blocks, but the actual 51-byte frame contains
one compressed block with three sequences and RLE tables. It must not be
cited as independent evidence of cross-block entropy reuse.
