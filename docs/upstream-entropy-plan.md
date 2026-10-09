# Module 8 upstream entropy coverage

This plan separates executable evidence from the remaining test migration.
The reference is facebook/zstd v1.5.7 revision
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`. It does not redefine the production
design in [entropy-encoder-design.md](entropy-encoder-design.md) or close the complete-migration gate.

## Test data and ownership

The current tests borrow immutable fixture descriptions and sources, own their
encoded buffers in `Vec<u8>`, and inspect wire positions with checked indices.
The standalone C oracle owns bounded input/output buffers in an external
temporary process. It is never linked into the Rust library. Selector
records contain scalar booleans, depths, capacities and source ranges; they
need no artificial unsafe backend. The existing checked `DataProducer`
preserves the original tail-consumption order without non-owning pointers.

The completed work was decomposed before implementation into exact selector
vectors; standalone depth/capacity contracts; the bounded adapted driver; a
separately pinned complete corpus replay; original dictionary/sequence
schedules; and implemented strategy adapters. Each was a separate reviewable
checkpoint. The sections below record their executable scope and retained
upstream obligations, rather than treating fixture presence as test coverage.

## Exact Huffman target boundary

The upstream target is
[`tests/fuzz/huf_round_trip.c`](https://github.com/facebook/zstd/blob/f8745da6ff1ad1e7bab384bd1f9d742439278e99/tests/fuzz/huf_round_trip.c).
Its single inventory case remains partial.

| Source lines | Original contract | Current evidence or remaining work |
| --- | --- | --- |
| 27–38, 68–80 | Histogram, highest nonzero symbol, and adjusted requested table depth | The driver preserves the original histogram and highest-symbol adjustment: `max(requested, ceil(log2(max_symbol + 1)) + 1)`. Independent C vectors also verify the cheap-depth arithmetic and its two-byte unsigned corner. |
| 43–57 | Tail selectors, in exact order | Exact selectors and source-prefix ranges are checked against independent C vectors under both BMI-dependent byte-consumption layouts. CPU flags are recorded observations, not invented Rust engine controls. |
| 55, 58–61 | Capacity in `0..=4*original_input_len`, remaining source capped at 256 KiB | The driver uses the original length before selector consumption/capping, with an explicit 1 MiB harness input limit. Its standalone encoder accepts sources through 256 KiB and checks guarded caller destinations; the production frame-block limit remains 128 KiB. |
| 63–77 | Empty/single-byte and uniform-source early exits | Separate normal witnesses and full-corpus outcome records cover all three exits after the original selector consumption. |
| 90–93 | Heuristic or optimal requested-depth selection, then successful table construction with log at most twelve | Cheap-depth selection matches the pinned scalar heuristic. Standalone Rust construction supports a hard cap through twelve; the optimal branch uses an independent complete-description/weighted-bit cost search. C tree identity and early-stop policy are not claimed. New inline trees remain limited to eleven. |
| 94–99 | Write the table into the selected destination; accept a reported failure | Rust copies the paired selected description with an exact-fit contract and explicit capacity failure, then reuses that same buffer for the stream. Independent C comparisons document that this differs from C's destination-dependent raw/FSE serialization and possible partial writes. |
| 100–108 | Read the written table through X1 or X2; explicitly fall back on the specific X2 table-log error | Rust reads back the exact selected description with one checked table representation. The separate same-description C oracle executes both table loaders and implements the specific X2 error fallback; the Rust corpus driver does not claim distinct X1/X2 engines. |
| 113–127 | Encode 1X/4X; when nonzero, decode exact source length and bytes | Thirty stored tables reproduce both C streams byte for byte. Live C checks cover stream bytes and capacity/zero-return boundaries; the complete adapted driver verifies exact output or a documented zero result for every selected source. |

The selector order is streams, decoder variant, optional BMI2 bit, optimal
depth, prefer-repeat, suspect-uncompressible, disable-assembly, disable-fast,
capacity, and requested depth. The BMI2 selector consumes a byte only when the
C host reports BMI2. No prefix-reservation selector precedes this sequence.
The remaining source begins at the original input start, not at the consumed
tail. The portable driver executes both consumption layouts and
compares them with independent C-produced parameter vectors.

`optimalDepth` changes `HUF_optimalTableLog` in the original target. The Rust
driver exercises its independent estimated-size policy; this is a meaningful
adaptation, not a claim of identical C optimal-table selection.
`preferRepeat` and `suspectUncompressible` are still consumed, but their policy
branches are in `HUF_compress_internal`, which this target never calls. They
must not be advertised as missing active controls for this particular target.
BMI2, X1/X2, disable-assembly and disable-fast choose C implementations or
table layouts. Independent C comparisons exercise host-supported selections;
no Rust knobs are invented for engines the Rust implementation does not have.
Both selector-consumption layouts run on every Rust host, which does not imply
that an unavailable native BMI2 engine was executed.

The caller-output adapter preserves the original minimum destination guards:
1X returns zero below eight bytes, and 4X below seventeen bytes or below twelve
source bytes. It also preserves native-word slack and every quarter's 16-bit
size bound, including the fourth quarter. These are C compressor contracts,
not wire-format minimum sizes. Rust's successful
six- and nine-byte 4X cases have an empty final segment and a marker; the C
oracle verifies them by assembling four independent 1X quarters. Do not call
that a successful invocation of the original C 4X branch. Similarly, the empty
1X `01` marker is checked without claiming a successful C zero-output decode.

The standalone tests preserve the full 256 KiB source contract without
truncating it to the production frame-block limit. Large 4X streams that exceed
the sixteen-bit quarter-size limit return an explicit zero result through the
capacity adapter. A standalone stream is never split into independent frame
blocks to bypass that contract.

## Executed evidence

### Complete pinned Huffman replay

The matching adapted driver now replays every one of the 833 pinned files under
both BMI consumption layouts. Safe and all-feature release builds agree on all
1,666 per-file records, for 3,332 total executions with no skipped files. The
archive holds 2,350,233 bytes; independent ZIP CRC, SHA-256 and exact extracted
member comparisons passed. Both layouts reach every requested and actual tree
depth from one through twelve.

| Driver outcome | Without BMI consumption | With BMI consumption |
| --- | ---: | ---: |
| Empty | 41 | 48 |
| Single byte | 7 | 2 |
| Uniform | 4 | 5 |
| Table unavailable | 0 | 0 |
| Description capacity failure | 74 | 72 |
| Zero stream | 122 | 91 |
| Exact roundtrip | 585 | 615 |

The corpus's maximum selected source is 238,251 bytes; normal witnesses
separately reach the full 256 KiB limit and the unavailable-table branch.
Maximum selected capacity is 631,680 bytes and maximum successful stream
length is 129,430 bytes. The two transcript fingerprints are respectively
`260763a2415eaf9d` and `7f5e2d10e7c0c952` in both backend configurations.

Commands, SHA-256 values, flags, depth/capacity histograms and qualifications
are preserved in [the replay evidence](evidence/2026-10-09-huffman-corpus.json).
All per-file records are archived as
[compressed JSON Lines](evidence/2026-10-09-huffman-corpus-records.jsonl.gz);
the evidence records both compressed and uncompressed hashes, and decompression
was checked against every original record before committing. Python's `gzip`
module or `gzip -dc` reads the complete transcript without the external corpus.
These are test metadata, not redistributed corpus inputs. Independent tree
policy, description copying and C engine qualifications below still apply.

### Standalone driver checkpoint

The driver in
[`tests/upstream/huffman_round_trip.rs`](../tests/upstream/huffman_round_trip.rs)
applies the original highest-symbol adjustment:
`max(requested, ceil(log2(max_symbol + 1)) + 1)`. For the non-optimal branch,
it preserves `FSE_optimalTableLog_internal(..., 1)` from v1.5.7: limit by
`floor(log2(source_len - 1)) - 1`, raise to the lesser of
`floor(log2(source_len)) + 1` and `floor(log2(max_symbol)) + 2`, then clamp
to 5 through 12. The two-byte C unsigned-subtraction corner is handled
explicitly without Rust overflow. The optimal branch applies the independent
Rust complete-description-plus-weighted-bits search under the adjusted cap;
a normal witness proves that this selector changes actual table selection.

After histogram validation, the driver builds and pairs a standalone table,
copies its exact description into the selected guarded destination, reads the
description back, and reuses the same destination for 1X/4X output. Successful
streams reconstruct the complete selected source. Empty, single-byte, uniform,
unrepresentable-table, description-capacity failure, zero-stream and successful
roundtrip outcomes are recorded separately. Normal tests witness all seven
outcomes, including the unavailable-table branch absent from the corpus. An
unexpected codec or allocation error fails the case instead of skipping input.

The caller-output adapter preserves caller bytes on failure, a stronger Rust
guarantee than C's internal partial writes. Its native-word threshold formula
is tested for 32- and 64-bit word sizes; live C comparisons validate the host
width. The paired-description copy intentionally does not reproduce C's
destination-dependent raw/FSE serialization choice. CPU flags and X1/X2
remain qualified engine selectors supported by separate independent C checks;
the complete corpus replay does not promote this source to full parity.

`tests/upstream/huffman_encode.rs` has thirty exact 1X and thirty exact 4X
fixture comparisons plus five explicit C oracle tests. The first three checks
passed safe and all-feature builds with 85 same-table invocations, 55
sparse-builder invocations and 59 Rust-4X invocations. Two later checks cover
48 standalone descriptions under both depth policies with 96 exact 1X/4X
streams, and six standalone streams through 256 KiB with 393,217-byte 1X output
and exact 65,535-byte 4X jumps. They also passed safe and all-feature builds.
These counts overlap in source tables and are not independent upstream cases.
The maximum prepared-log-12 4X case at the production 128 KiB block limit
encodes to 196,618 bytes with three 49,153-byte jump entries.

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

## Completed original dictionary and sequence campaigns

### Exact incremental sequence source and line 1551 codec sweep

[`tests/upstream/sequence_generator.rs`](../tests/upstream/sequence_generator.rs)
implements the three shapes from unchanged `tests/seqgen.c` with a typed phase,
checked output indices and one `0x87654321` random state across the full
schedule. Match-length requests produce a different guard byte, `value+1`
copies of the match byte, and a final different guard. Literal-length requests
save the random state, produce `min(value,128)` high-alphabet bytes, the remaining
low-alphabet literals, and replay the saved high-alphabet prefix. Offset requests
use zero padding instead of low-alphabet literals. Unsigned wrapping, saved-state
replay, unfinished-request identity and zero-capacity phase changes match C.
All 971 stored per-request C vectors preserve byte counts, request/cumulative
XXH64 hashes and seeds. Live comparisons also check 6,459 fragmented calls,
including output capacities 0, 1, 2, 127, 128, 129, 1,023 and 1,024.

[`tests/upstream/sequence_sweep.rs`](../tests/upstream/sequence_sweep.rs) runs
all three kinds for values 0..299, each power 2^8 through 2^16 and its
quarter-offset value, then offsets `2^log-1` for every log 8..24. This is the
complete `zstreamtest.c:1551-1599` source schedule: **971 requests**,
**34,639,743 bytes**, **34,593 generation calls**, final XXH64
`d6a9522c4b14b2a5` and final random seed `0f900e80`. It uses 1,024-byte source,
compression-output and decompression-output buffers, one persistent prepared
dictionary and one frame, with Continue until final Finish. A second generator
compares every decoded byte incrementally without retaining the full source.
All three Rust strategies check counters, output canaries and finalization.

The original C context inherits explicit level-5 parameters from the advanced
initializer at `:1444`, followed by a level-3 request, two workers and a 4 MiB
job size at `:1506-1508`. The `:1568` session-only reset preserves those
settings. Although the new CDict requests window log 24, the emitted frame
advertises **window log 21**; Rust matches that actual window. The original
decoder maximum and complete offset schedule still reach log 24. The oracle
reconstructs all of these controls instead of starting from a fresh level-3
context. Rust's named policies are independent and single-threaded: numeric
level 3, borrowed CDict ownership and the inherited multithreaded setup remain
C oracle evidence, not Rust API parity.

Three normal sweep tests passed in each of safe, all-feature, no-default and
twelve individual unsafe-feature configurations: **45 complete strategy
campaigns**. Safe and all-feature release C checks each decoded the complete
34,639,743-byte C baseline with Rust and three Rust frames with C, totaling
103,919,229 Rust-to-C output bytes per configuration. An isolated temporary
multithreaded C-library build also passed the oracle. Frame observations are:

| Encoder | Frame bytes | Complete frame XXH64 |
| --- | ---: | --- |
| Original C with inherited controls | 350,954 | `edb8a88a3d01d60a` |
| Rust Greedy | 348,912 | `dbb1ad7ed4898eea` |
| Rust Chain | 349,055 | `e125a29d7329804b` |
| Rust Lazy | 348,567 | `3a7548eab319e174` |

These frames need not be byte-identical across strategies. Rust's bounded
128 KiB search history does not guarantee choosing every long match suggested
by the source generator. This campaign checks every original source byte; it
does not claim that every OF code is emitted by each Rust strategy.

### Original entropy lifetimes at lines 1831 and 1884

[`tests/upstream_entropy_lifetimes.rs`](../tests/upstream_entropy_lifetimes.rs)
uses the exact 49,152-byte default trained dictionary, including serialized
header bytes in the original source ranges. Its
[fixture provenance and reproduction notes](../tests/upstream/entropy-lifetimes/README.md)
record the training source, both earlier RNG paths and C frame parameters.

At `zstreamtest.c:1831`, both preceding window-sweep modes run: 339,966 RNG
draws yield seed 437,141,494, and 399,960 draws yield seed 3,173,672,436.
Each source contains 131,072 RDG(0,0,seed) bytes, 130,816 fresh RDG(.05,0,seed)
bytes, then serialized dictionary ranges `[256..384]` and `[128..256]`.
The complete source is 262,144 bytes. C's initial dictionary parameters report
window log 17, but the single-segment output advertises 262,144 bytes; Rust
streaming uses log 18. Greedy, Chain and Lazy preserve the two-block boundary,
raw first block, compressed second block, exact decoded bytes and demonstrated
dictionary-content dependence. Each safe/all-feature C run checks two original
C frames with Rust and six Rust streaming plus two one-shot frames with C.

At `:1884`, every one of **43,691** Flush operations submits six `0xaa` bytes,
including the original two-byte overshoot. Nine 128-byte serialized dictionary
ranges follow, with offsets 1,024 down to zero: eight Continue operations and
one Finish. Total source size is **263,298 bytes**. C's actual frame window is
2 MiB/log 21 despite initial CDict parameters of log 17; Rust matches log 21.
The original C frame has 43,691 raw small blocks and one final compressed block;
Rust emits RLE small blocks but preserves every boundary and all eight buffered
Continue operations. Tests inspect final sequence modes and verify dictionary
content dependence, rather than relying solely on roundtrip success. Each
safe/all-feature C run checks the original frame with Rust and all three Rust
strategy frames with C, totaling 789,894 Rust-to-C decoded bytes.

The normal lifetime tests passed safe, all-feature and no-default builds;
the six-byte flush case also passed each of the twelve individual unsafe
features. These default-profile campaigns do not implement numeric C level 3,
borrowed by-reference CDict ownership or upstream's nondefault training profiles.

## Completed named-strategy corpus adaptation

The regression adapter retains its bounded sorted file plan and one-shot
baseline. Its separate
[`external_regression_corpus_named_strategies`](../tests/upstream_regression_corpora.rs)
runner reuses one encoder for each of Greedy, Chain and Lazy, resets between
complete files, pledges the original size and drains Finish through guarded
caller-owned output. It checks progress, total counters and every decoded byte.
Length-delimited full-source and full-frame fingerprints make backend results
comparable without requiring different strategies to emit the same frame.

All **512** pinned Silesia/GitHub directory files, **12,407,963 bytes** per
strategy, passed under safe and all-feature release builds with no skips.
Every strategy's source/frame fingerprints, byte counts and retained capacities
agree between builds. The [recorded evidence](evidence/2026-10-09-strategy-corpora.json)
and [regression scope](../tests/upstream/REGRESSION.md) preserve commands,
hashes and exact dataset limits. This is functional content replay, not a
throughput benchmark or the original compressed-size regression matrix.
The separately downloaded GitHub dictionary and complete tar-byte datasets
are not substituted for their still-unported method branches.

## Remaining upstream obligations

- Keep the Huffman target partial for independent optimal-tree policy,
  exact-fit selected-description copying instead of C's capacity-dependent
  serialization, and the single Rust decoder instead of X1/X2/CPU engines.
  A successful full corpus run does not remove these distinctions.
- `fuzzer.c:4190` still requires C's caller-owned normalized-header destination
  behavior. Huffman caller-output support does not provide that FSE API.
- `fuzzer.c:1398` requires level 19, minimum match seven and post-sequence
  splitting; `:3265` uses the level-19 optimal parser with repeat offsets
  10/10/10. Independent Chain/Lazy policies and duplicate-repeat unit tests
  do not implement those original strategy contracts.
- Block-splitter, superblock, target block-size, numeric-level, long-distance
  matching, context-copy/static workspace and multithreaded encoder groups
  remain blocked or partial until their actual contracts are implemented.
- The regression suite still lacks the complete C method/configuration matrix,
  original numeric levels, dictionary and CLI branches, exact streaming-method
  schedules, complete tar inputs and historical compressed-size CSV comparisons.

The full upstream migration gate remains open. Completed source schedules and
content assertions are kept distinct from unavailable API setup and unsupported
engine or strategy branches; historical validation reports remain unchanged.

The byte fixture at `zstreamtest.c:1743` is already a complete decoder port.
Its comment describes three blocks, but the actual 51-byte frame contains
one compressed block with three sequences and RLE tables. It must not be
cited as independent evidence of cross-block entropy reuse.
