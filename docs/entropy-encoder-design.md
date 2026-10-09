# Module 8: entropy encoding and bounded compression strategies

This design precedes Module 8 implementation. It is a plan, not a statement that
the described APIs, features, or upstream test coverage already exist. The
coordinator records the executable decomposition in `TODO.md` and commits each
verified item separately. Source, documentation, and tests use English.

The reference is facebook/zstd v1.5.7, commit
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`. Relevant counterparts are
`BIT_CStream_t`, `HUF_CElt`, `HUF_readCTable`, `HUF_writeCTable_wksp`,
`FSE_CState_t`, `FSE_buildCTable_wksp`, `ZSTD_selectEncodingType`,
`ZSTD_encodeSequences`, `ZSTD_compressedBlockState_t`, and the hash-chain/lazy
search roles in `zstd_lazy.c`. Their wire contracts guide the implementation;
their packed layouts and pointer relationships do not define Rust structures.

## Scope and compatibility

Add Huffman literals, adaptive and reused sequence distributions, repeat-offset
selection, and bounded alternatives to the current single-candidate greedy
search. Keep the existing raw and RLE fallbacks. Compression remains deterministic
for identical options, input, and dictionary across safe and unsafe backends.

Preserve these Module 7 contracts:

- Dictionary training still builds a complete 256-symbol alphabet with its
  current pseudocount, depth-limit, and serialization fallback policy. A new
  sparse literal builder must not silently change trained dictionary bytes.
- Encoder output history remains bounded by `min(advertised_window, 128 KiB)`.
  Better candidate search does not imply retention of the entire advertised
  window or the entire frame. Dictionary ownership and existing match-source
  eligibility checks remain in force.
- Dictionary references are allowed at a sequence's match start while frame
  output position is at most the advertised window. Ordinary output matches
  obey the window, retained-history interval, and overlap rules. Entropy reuse
  has frame lifetime and does not expire with dictionary match eligibility.
- Prefixes apply to one standard frame; persistent dictionaries survive reset.
  Skippable frames do not consume a prefix. Failed contexts still require reset.

All core code remains compatible with Rust 1.89 and `no_std` plus `alloc`.
Typed storage is owned by arrays or fallibly reserved `Vec`s; shared dictionaries
remain immutable owned handles. No C codec is linked into the implementation.
Allocation boundaries remain explicit for future allocator parameterization,
without assuming a particular future `Allocator` API.

Expose only implemented named strategies. The proposed names are `Greedy`
(current single-candidate matching), `Chain` (bounded multi-candidate greedy
matching), and `Lazy` (bounded candidates with one-byte lookahead). Document
their actual search budgets and memory use when implemented. Do not accept
upstream numeric compression levels, advertise `zstd_fast`/`zstd_lazy` parity,
or imply optimal parsing, long-distance matching, or threading.

## Shared forward bit writer

Extract the existing `encode.rs::BitWriter` into a reusable internal component
before changing its behavior. It owns an initialized `Vec<u8>`, a `u64` pending
reservoir, and a pending-bit count smaller than eight between writes. Input fields
have width at most 32 and values that fit that width. Zero-width writes require
value zero. Checked reservation precedes mutation; a write emits at most four
bytes. Closing writes the terminal one bit and the remaining partial byte with
zero high padding. Reset clears logical contents while retaining capacity.

Callers encode source symbols or transitions in reverse order as required by
their format. The writer merely appends each numeric field little-endian; it
must not reverse a Huffman codeword's individual bits. A stream with no symbols
still has the one-byte terminal marker. Repeated close/write-after-close must
either be prevented by the internal API or rejected explicitly; no caller may
accidentally append a second marker.

The checked backend uses normal vector operations. `unsafe-bitstream` remains
the independently selected writer backend: successful reservation proves the
capacity and checked length growth, writes initialize exactly the new prefix,
and `set_len` occurs after the exclusive spare-capacity borrow ends. Extraction
first requires byte-identical output against the existing encoder baseline.

## Huffman codebooks and table descriptions

`Codeword { value: u16, bits: u8 }` is a plain initialized value. `bits == 0`
means that a symbol is absent. `HuffmanCodebook` owns 256 such entries and the
validated maximum code length. It does not borrow a decoder's table or point
into its own storage. A setup conversion from an existing immutable decoding
table may copy its symbol/code relation, including a prepared dictionary's
table; no borrow escapes that conversion.

For validated code lengths `L[s]`, let `log` be the maximum nonzero length.
There must be at least two present symbols and exact Kraft mass:

```text
sum(1 << (log - L[s])) over present symbols == 1 << log
weight[s] = log + 1 - L[s] for present symbols, otherwise 0
```

Assign codes in ascending weight, then ascending symbol order, matching the
decoder's table construction. Starting with `rank_start = 0`, a symbol with
weight `w` occupies `copies = 1 << (w - 1)` entries and receives
`value = rank_start >> (w - 1)` and `bits = log + 1 - w`; advance `rank_start`
by `copies`. Validate final mass, alignment of every code range, and code width
before publishing the immutable codebook. Equivalently, an exact validated
decoder table can be inverted from each symbol's first lookup index. The example
lengths `[1, 2, 3, 0, 4, 4]` yield codes `1, 01, 001, absent, 0000, 0001`.

`BuiltLiteralTable` owns both the selected codebook and its exact serialized
description. This pairing is mandatory: the current dictionary serializer may
replace learned lengths with `near_flat_lengths` when the FSE weight payload is
too large. Encoding against the original lengths after that substitution would
be incorrect. Build both fields from one final selection, or reconstruct the
codebook from the exact validated description. Do not rerun two independent
selection procedures at the call sites.

The new sparse literal builder counts only present symbols. Zero literals use
raw representation; one distinct literal uses RLE. Neither case invents a
one-symbol Huffman tree. For larger alphabets, preserve every absent symbol's
zero weight below the highest present symbol, and omit that last symbol's
weight. If the highest present symbol is at most 128, a direct description is
possible: the first byte is `127 + explicit_weight_count`, with weights packed
high nibble first and a zero unused final low nibble. This handles one explicit
weight and constant explicit-weight lists, which the two-state FSE writer does
not encode. Otherwise use a valid FSE payload shorter than 128 bytes. Any
fallback must rebuild the paired codebook and description together; raw literals
remain a valid alternative when a description cannot produce a useful saving.

New inline literal tables have maximum code length eleven. A validated prepared
dictionary may supply a twelve-bit table, including the official HTTP fixture.
Such a table can encode treeless literals; do not re-emit it as a new inline
eleven-bit-limited table. Before considering reuse, require a nonzero code width
for every input literal. Missing symbols make reuse ineligible, not an excuse
to emit zero bits for that symbol.

`unsafe-huffman-codebook` selects only encoding-codebook access. The existing
`unsafe-huffman` continues to select decoder access. Both encoding backends
perform the same eligibility and width checks. The unchecked accessor proves
that the symbol index addresses an initialized entry in live, correctly aligned
owned storage; the shared borrow forbids mutation/reallocation during the read,
and copied records contain no ownership or escaping references. A separate
unsafe representation for the scalar `Codeword` itself would serve no purpose.

### Literal streams and headers

Encode each stream's symbols in reverse order, then add its marker. The known
two-symbol fixture `[0, 1, 1, 0]` encodes to byte `0x16`. Four streams use
`q = ceil(regenerated_size / 4)` output symbols in the first three streams and
`regenerated_size - 3*q` in the fourth. Four-stream input requires at least six
literals. At sizes six and nine the last stream is empty but still emits `0x01`.

The jump table stores the first three compressed stream sizes as little-endian
`u16`s. Each size is positive and checked before conversion. The fourth size is
the nonempty remainder after the six-byte jump table and the first three streams.
With a 128 KiB block and at most twelve bits per symbol, each quarter's worst
case is bounded below `u16::MAX`; enforce the bound rather than relying on a
compression heuristic.

Standalone entropy tests use a separate 256 KiB source limit without changing
frame literal limits. Four-stream preparation sums actual code widths plus each
terminal marker before mutating scratch. Every leading stream must fit `u16`;
using the table's maximum width alone would incorrectly reject large sources
made of short codes. Exact jumps of 65,535 bytes are accepted, while overflow
preserves both scratch buffers. A twelve-bit standalone 1X stream may occupy
393,217 bytes. Six independent C cases cover the larger source limit, short
codes in a log-twelve table, maximum one-stream size, and exact jump bounds.

| Size format | Streams | Regenerated and compressed size fields | Header bytes |
| --- | --- | --- | --- |
| 0 | 1 | Each at most 1,023 | 3 |
| 1 | 4 | Each at most 1,023 | 3 |
| 2 | 4 | Each at most 16,383 | 4 |
| 3 | 4 | Each fits 18 bits; regenerated size still at most 128 KiB | 5 |

Compressed size includes the optional tree, jump table, and terminated streams,
but excludes the literals header. Select a header using both sizes. More than
1,023 regenerated literals cannot use one stream. A small input can also require
a larger four-stream header because its encoded size crosses a field boundary.

Choose among raw, literal RLE, a new table, and eligible reuse using complete
serialized costs, including headers. A literal-only compressed block is legal:
write a sequence count of zero and no sequence modes or sequence bitstream.
Remove the current assumption that an empty sequence list always means raw
block fallback. Compare the complete block payload against raw/RLE alternatives.

## Generalized inverse FSE tables

`FseEncodingTable` owns initialized inverse-transition entries, initial-state
indices, symbol availability, and the actual table log. It replaces the
predefined-only inverse table without adopting C's type-punned layout.
Construction validates a decoding table, then inverts its checked relation:
an entry for symbol `s`, base `b`, and bit width `n` reaches successors
`b..b + (1 << n)`. Every present symbol must cover each successor state exactly
once. Detect collisions, missing transitions, invalid ranges, and invalid
normalized mass before publishing the table.

Absent symbols have explicit unavailable initial states and transitions.
Both initialization and transition lookup reject them before reading an entry;
zero-filled storage is not a valid substitute for an absent transition. Array
dimensions and multiplication are checked before allocation. Sequence limits
remain LL symbols `0..=35` with log at most nine, OF `0..=31` with log at most
eight, and ML `0..=52` with log at most nine. Existing match-source distance
limits are additional restrictions; an OF code's representability does not
promise a larger search horizon.

An explicit inline RLE distribution has one state and table log zero. All
transitions emit zero bits and only its sole symbol is accepted. This differs
from a prepared or repeated normalized table containing one symbol at nonzero
log: preserve that table's actual state width and initial-state bits. Never
collapse a repeated dictionary table into inline RLE semantics.

For nonzero sequence counts, each LL/OF/ML channel may select predefined, RLE,
new normalized, or repeat mode. New distributions use the existing checked
normalizer and normalized-count serializer within the channel's limits. Preserve
the support of every sequence symbol. Evaluate a bounded set of candidates with
deterministic ties and complete description/state/extra-bit costs; do not claim
upstream normalization or mode-selection identity. A zero-sequence block leaves
all three FSE channels unchanged and has no modes byte.

Plan sequences forward, then encode their symbols in reverse order. Last-symbol
initialization selects actual states for the final LL/OF/ML symbols without
inventing a last transition. Preserve the decoder's initial-state order, extra
bit order, transition order, and exact terminal consumption. Reused tables must
cover the chosen terminal symbols and every preceding symbol.

`unsafe-fse-encoder` selects inverse table access independently of `unsafe-fse`.
Migrate the old inverse accesses out of `unsafe-fse`; decoding-table construction
and access keep that existing feature. The checked wrapper validates symbol
availability, successor state, and flattened index before unchecked access.
The local proof covers initialized length, checked arithmetic, alignment,
allocation lifetime, and immutable shared access. Transition and state records
remain ordinary safe values. Test safe decoding with unsafe encoding and the
opposite combination, not only an all-feature configuration.

## Repeat planning and transactional entropy state

`PlannedSequence` stores literal length, match length, resolved distance,
selected offset value/code, and extra bits as plain values. A forward planner
starts with the committed repeat triple, checks each match, chooses its wire
offset value, and computes a candidate next triple. It does not mutate committed
state while comparing parses or encoding candidates. Dictionary repeat triples
are used exactly, including legal duplicates.

For offset values one through three:

| Literal length | Value 1 | Value 2 | Value 3 |
| --- | --- | --- | --- |
| Nonzero | rep0; triple unchanged | rep1; swap first two | rep2; move selected offset to front |
| Zero | rep1; swap first two | rep2; move selected offset to front | rep0 minus one; move result to front |

The subtract-one result must be nonzero. An explicit offset value is
`distance + 3`, calculated without overflow, and updates the triple to
`[distance, old_rep0, old_rep1]`. It remains available when repeat candidates
are unsuitable. Multiple encodings of the same distance, especially with
duplicate initial repeats, require deterministic selection and their own exact
resulting triples. Never infer the triple solely from the resolved distance.

`EmittedEntropy` owns the Huffman and three FSE tables established by the last
accepted encoded block, plus the repeat triple. At frame start these derive
from the selected formatted dictionary, or default repeats `[1, 4, 8]` with
no reusable entropy tables. Tables can be prepared lazily from the immutable
dictionary without retaining self-references.

`PendingEntropy` describes a candidate's replacements and repeat result. Reuse
can be expressed by a value tag identifying the current channel; it need not
clone whole tables or retain references into mutable context storage. Only the
accepted candidate transfers new owned tables into emitted state. Each FSE
channel is updated independently. Raw and whole-block RLE fallback preserve all
entropy/repeat state. Raw/RLE literals inside an otherwise compressed block
preserve Huffman state while its nonempty sequences may update FSE/repeats.

Complete construction and fallible output reservation before appending the
selected block and publishing its pending state. Here "emitted" includes bytes
accepted into the encoder's owned output queue; it does not require the caller
to have drained that queue. No fallible mutation may leave published tables
describing discarded bytes. Failures leave the candidate unpublished and follow
the existing context failure contract.

Treeless reuse means the most recent emitted Huffman table. After an inline table
replaces the initial dictionary table, the encoder cannot silently return to
that dictionary table with another treeless block. Repeat FSE modes obey the
same per-channel rule. Frame reset restores initial dictionary/default state;
block boundaries and flush calls alone do not.

`SequenceEncoder` implements those roles with three current and three spare
owned table slots, scalar availability tags, and three lazily cached predefined
tables. A successful preparation owns the complete sequence section; commit is
infallible and idempotent, including for zero sequences. Discard and reset clear
availability without freeing any slot or scratch buffer. Every retained table,
plan, symbol vector, stream and section is included in capacity accounting.
Constructing a new immutable candidate or installing a prepared dictionary may
replace a bounded slot with a fresh allocation; retaining workspace does not
claim that all subsequent table construction is allocation-free.

## Bounded candidate storage and lazy parsing

`MatchChain` owns a fixed-size hash-head array and a bounded initialized node
array. Nodes contain source positions, predecessor indices, and reuse tags;
none contains a pointer to input, dictionary, or history. Dictionary candidates,
retained output, and current block positions resolve through the existing
checked match-source view. Logical frame positions remain `u64`; physical node
indices and bounded slice positions are validated before `usize` conversion.

Every link either denotes an older initialized node or is empty. If node slots
are recycled, a checked generation/position tag must distinguish stale links;
an old index must never silently identify a newer node. Tag rollover requires
checked rejection or an explicit whole-index reset. Cycle/staleness checks and
a fixed visit budget terminate every search independently of input bytes.
Hash equality identifies candidates only; verify source bytes and all
dictionary/window/history bounds before accepting a match.

The proposed initial policy has 65,536 hash heads, at most 262,144 node slots,
at most 65,536 retained dictionary seed positions, eight candidate visits per
`Chain` search, and sixteen visits at each of `Lazy`'s two search positions.
These are independent Rust policy choices, not upstream level equivalents.
Record any adjustment during review before implementing its checkpoint. Allocate
the new node storage only for strategies that use it. These caps are independent
of total frame length and do not enlarge the 128 KiB output-history policy.
Retain allocations across resets, report their high-water capacity, and ensure
evicted source positions cannot be dereferenced. A longer chain may improve the
compression ratio, but bounded seed retention remains a search limitation.

The public `CompressionStrategy` value is selected through
`EncoderOptions::strategy`; Greedy remains the default for streaming and
one-shot helpers. Chain and Lazy use the same checked source integration. Reset retains
the strategy; `reset_with_options` may change it for the next frame. Previously
allocated storage remains counted and reusable when switching policies.
On 64-bit targets the working chain owns 9 MiB and its optional dictionary seed
owns 3 MiB. Entropy, input/output buffers, the bounded ring and shared dictionary
content add to those figures. Greedy does not allocate a chain unless that
context previously selected a chain-based strategy.

`Chain` compares a bounded set of candidates at the current position. `Lazy`
also examines one later input position and chooses using a documented
deterministic rule accounting for its additional literal. Tentative lookahead
must not mutate repeat or entropy state. If lookup seeds future positions,
every subsequent match still enforces `candidate_position < match_start`.
Dictionary eligibility is evaluated at the actual chosen match start, including
the exact `position == window` boundary. Source advancement, inserted positions,
and trailing literals must cover the input exactly once.

Lazy searches the current and next positions with sixteen visits each. It
inserts the current position before probing the next, enabling distance-one
overlap, but leaves the next uninserted until selection. It delays only when
`next_length > current_length + 1`; ties keep the current match. A full-tail
match skips lookahead. Both choices insert every consumed valid key once.
The constructed dictionary witness produces frames of 369 bytes with Chain
and 279 bytes with Lazy on the initial public integration, a reduction of
about 24.4%. This finite witness demonstrates a benefit, not a general ratio
guarantee; fixed frame overhead reduces the percentage compared with the
private parser's complete literal/sequence payload comparison.

`unsafe-match-chain` independently selects head/node access. It does not replace
the existing `unsafe-match-table` backend. Validate hash, node index, initialized
range, and tag before any unchecked read; mutable updates require a unique
borrow and must not hold overlapping mutable references into head/node storage.
Validation and source comparisons stay identical in safe and unsafe versions.

## Independent verification and measurement

Format tests include malformed or absent codes, normalized support holes,
dictionary log twelve, reused single-symbol nonzero-log distributions, every
repeat branch, duplicate repeats, zero-sequence compressed blocks, and candidate
fallback without state publication. Exercise new/repeat/raw/RLE block sequences,
independent FSE channel replacement, frame resets, prefixes, and tiny streaming
destinations. Force codebook-description fallback and compare the exact decoded
weights with the codebook actually used.

For Huffman boundaries use lengths zero, one, five, six, seven, nine, eleven,
twelve, 255/256, 1,023/1,024, 16,383/16,384, and 128 KiB. Exercise encoded-size
field boundaries separately. The pinned C X1 and X2 decoders accept four-stream
sizes six through eleven, including an empty final stream; upstream's compressor
refusal below twelve is a savings policy, not the format's lower bound.

Independent C executables remain test oracles only:

1. Read each exact Rust description with `HUF_readStats` and `HUF_readCTable`.
   Compare weights/lengths and use that C table to encode identical 1X/4X inputs.
   Compare stream bytes for the same table, then decode Rust output using both
   C X1 and X2 tables. Give C enough destination slack; its internal minimum
   buffer requirement is not a format-size requirement.
2. Re-encode the existing thirty official Huffman fixtures from their exact
   descriptions and compare the independent stream bytes. For the HTTP
   dictionary, separately cover actual twelve-bit symbols and all fourteen
   missing symbols. Verify successful reuse through treeless full frames with
   the official dictionary decoder. Keep inline log-twelve rejection separate.
3. Validate new FSE descriptions with `FSE_readNCount`, exercise inverse
   transitions against the independent relation, and decode complete sequence
   blocks/frames with official zstd. Matching upstream's adaptive tree or final
   compression bytes is not required when our independent selection differs.
4. Use a deterministic order-four de Bruijn sequence over sixteen symbols as a
   literal-only compression witness without repeated four-byte match keys.
   Require both exact decode and a real size reduction. Add collision, stale
   node, wraparound, long logical position, and lazy-choice witnesses for search.
5. Adapt `huf_round_trip.c`, entropy/sequence fuzzer cases, and supported strategy
   assertions with explicit mappings. The standalone upstream Huffman fuzzer
   can request new log-twelve tables and CPU-dependent flags; do not conflate
   those branches with dictionary-table reuse or claim unsupported optimizations.

Extend validation and benchmark matrices with `unsafe-huffman-codebook`,
`unsafe-fse-encoder`, and `unsafe-match-chain` individually and together. Verify
all existing features, `no_std`, Clippy, relevant Miri targets, and independent
oracles. Compare identical deterministic workloads and encoded fingerprints for
safe/unsafe variants of each strategy. Record throughput, compressed sizes,
retained memory, allocation reuse, seeds, toolchain, and variance. Strategy
comparison may change encoded bytes; backend comparison must not.

## Ordered implementation checkpoints

Each numbered item is an independently verified coordinator commit, with its
matching `TODO.md` item checked in that commit. Add finer decomposition before
an item grows beyond a reviewable change.

1. Extract the reusable forward bit writer with byte-identical baseline output.
2. Add the checked canonical codebook and exact serialized-description pairing.
3. Add `unsafe-huffman-codebook` with focused bounds, missing-symbol, and Miri checks.
4. Encode one Huffman stream and verify exact C streams, including dictionary log twelve.
5. Encode four streams and checked jump tables, including minimum and header boundaries.
6. Add sparse literal-table construction while preserving dictionary-training output.
7. Add checked literal headers and zero-sequence compressed-block assembly.
8. Introduce emitted/pending entropy state and publish Huffman choices transactionally.
9. Generalize checked inverse FSE construction, absent-symbol rejection, and explicit RLE.
10. Add `unsafe-fse-encoder` and isolate it from the decoder's `unsafe-fse` feature.
11. Plan repeat-offset values forward with exact candidate triples and overflow checks.
12. Encode sequences with supplied predefined/new/reused/RLE tables and checked terminal states.
13. Select adaptive per-channel distributions with bounded deterministic cost comparisons.
14. Publish FSE and repeat updates only with accepted blocks; verify dictionary and reset lifetimes.
15. Add checked bounded multi-candidate storage and stale-link/tag validation.
16. Add `unsafe-match-chain`, preserving candidate order and search limits.
17. Implement the named multi-candidate strategy with fixed documented budgets.
18. Implement one-byte lazy lookahead with independent candidate state and meaningful ratio tests.
19. Expose only completed named strategies and port their supported upstream assertions and corpora.
20. Extend backend/strategy benchmarks and validation, record remaining upstream gaps, then close the module.

Source anchors for review: `doc/zstd_compression_format.md` literals headers,
Huffman prefix assignment, sequence encoding modes, and repeat-offset sections;
`lib/compress/huf_compress.c::{HUF_readCTable,HUF_buildCTableFromTree,HUF_compress4X_usingCTable_internal}`;
`lib/common/fse.h::{FSE_initCState2,FSE_encodeSymbol}`;
`lib/compress/zstd_compress_sequences.c::{ZSTD_selectEncodingType,ZSTD_buildCTable,ZSTD_encodeSequences_body}`;
and `lib/compress/zstd_lazy.c::{ZSTD_HcFindBestMatch,ZSTD_compressBlock_lazy_generic}`.
