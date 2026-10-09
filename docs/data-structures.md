# Data structures and backend design

This is the design inventory for the rewrite, not a claim that every item is
implemented. `TODO.md`, `docs/progress.md`, and the source describe the current
implementation. Before introducing another structure, add its ownership,
invariants, and safe/unsafe comparison here.

## Reference and translation rules

The reference is [facebook/zstd v1.5.7](https://github.com/facebook/zstd/tree/v1.5.7),
commit `f8745da6ff1ad1e7bab384bd1f9d742439278e99`. The inventory was checked against
that checkout, especially `lib/common`, `lib/compress`, and `lib/decompress`.
The [format specification](https://github.com/facebook/zstd/blob/v1.5.7/doc/zstd_compression_format.md)
defines serialized data; C structure layout does not define the Rust layout.

Non-owning locations become indices or ranges into a separately owned or borrowed
buffer. A structure may hold a Rust slice borrow, but must not store an address
into another field of itself. Owned growable storage uses `Vec<T>`; fixed owned
storage uses `Box<[T]>`; shared immutable prepared objects may use `Arc<T>` where
pointer atomics are available. Counts from input are validated before conversion
to `usize`, allocation, shifts, or address calculations.

Safe and unsafe backends expose the same safe API and reject the same malformed
input. An unsafe backend changes internal access, not the wire format or caller
obligations. A feature selects only its own family. The default implementation
contains no unsafe operations. Plain records and state enums are shared: replacing
an integer with a raw pointer does not provide a meaningful alternative. Their
performance is measured through the storage or accessor backend that uses them.

## Initial codec inventory

Detailed module proofs and decompositions are in [entropy-design.md](entropy-design.md),
[huffman-design.md](huffman-design.md), and [frame-design.md](frame-design.md).

Official-test descriptors, source inventories, and expected-data records are
test-only plain metadata with owned strings/vectors and borrowed immutable
fixture slices. The test generator owns scalar PRNG state and uses indices for
repetitions. They introduce no unsafe access family; see
[the test migration design](upstream-test-port.md) and its linked inventories.

| Structure and upstream counterpart | Safe representation | Opt-in unsafe alternative | Invariants and focused comparison |
| --- | --- | --- | --- |
| Byte reader/writer; `MEM_readLE*`, `MEM_writeLE*`, `ZSTD_inBuffer`, `ZSTD_outBuffer` | Slice plus `usize` position; checked subranges and `from_le_bytes` / `to_le_bytes` | `unsafe-byte-cursor`: unchecked access or unaligned loads only after one complete range validation | `position <= len`; checked position arithmetic; no hidden padding. Compare tiny headers, full words, tails, and unaligned subslices. |
| Reverse entropy reader and forward entropy writer; `BIT_DStream_t`, `BIT_CStream_t` | Slice, bit position/count, integer reservoir; explicit terminal marker handling | `unsafe-bitstream`: unchecked refill/store after proving the whole byte range is valid | No shift by or beyond the reservoir width; no bits synthesized past the logical end; reader consumes the exact stream. Compare short streams, refill boundaries, and mixed bit widths. |
| Frame/block metadata; `ZSTD_FrameHeader`, `blockType_e` | `FrameHeader`, `BlockHeader`, enums, `Option<u64>` for absent content size | Shared value layout; byte access uses the selected cursor backend | Reserved block type rejected; sizes obey frame and block limits; absent size differs from zero. A scalar-only alternative would not test pointer access performance. |
| Output and history; `ZSTD_window_t`, `ZSTD_DCtx` prefix/dictionary fields | Owned `Vec<u8>` or caller output slice with a produced length and logical history indices | `unsafe-history`: checked wrapper plus raw copying/unchecked indexed copying | Offset is nonzero and within available history and allowed window; final output length is checked before writes; matches may overlap their destination. Compare offsets 1, 2, 3, 8, large offsets, and long matches. |
| Literal storage; `SeqStore_t` literal fields and `ZSTD_DCtx::litPtr` | Borrowed input range for raw literals; owned initialized byte buffer for decoded literals | Uses `unsafe-byte-cursor` for source access and `unsafe-history` for output copying | Literal cursor never exceeds regenerated literal count; block output and allocation limits apply before expansion. Borrowed source stays alive while referenced. |
| Sequence record/storage; `SeqDef`, `SeqStore_t`, `rawSeq`, `RawSeqStore_t` | Plain `Sequence` records plus `Vec<Sequence>` and positions; full-width validated lengths initially | Records remain shared; their execution uses `unsafe-history`; a separate future storage feature requires a benchmark before introduction | Literals and matches fit block/output limits; total lengths use checked arithmetic. Avoid C's compressed `u16` long-length side channel unless measurements justify it. |
| Repeat-offset state; `rep[ZSTD_REP_NUM]`, `ZSTD_compressedBlockState_t` | `[u32; 3]` or a wrapper with explicit update methods | Shared three-value record; history access is separately selectable | Initial offsets are `[1, 4, 8]`; zero-literal repeat-code rules and the subtract-one case are applied exactly; resolved distance must be valid before history access. No artificial unsafe array indexing is needed. |
| FSE normalized distribution and table; `FSE_DTableHeader`, `FSE_decode_t`, `ZSTD_seqSymbol` | Typed initialized entry vector/boxed slice; table log and validated normalized frequencies | `unsafe-fse`: unchecked entry lookup after proving `state < entries.len()` | Table size is `1 << table_log`; frequency mass including low-probability symbols is exact; every reachable transition and symbol is in range. Compare construction and symbol decoding separately. |
| FSE machine state; `FSE_DState_t`, `FSE_CState_t` | Numeric state plus a borrowed typed table or table identifier | Uses `unsafe-fse`; value fields remain shared | Table stays alive and immutable; decoded extra bits cannot cause an invalid next state. Encode and decode table layouts need not match C's type-punned allocation. |
| Huffman codebook and decode table; `HUF_CElt`, `HUF_DTable`, `HUF_CTableHeader` | Typed code/bit-length entries and initialized lookup tables; weights and rank arrays | `unsafe-huffman`: unchecked lookup after validating table and input-derived index | Weight sum, inferred last weight, maximum code length, and table completeness are validated. Four-stream boundaries are checked independently. Compare build, one-stream, four-stream, and tail decoding. |
| Match lookup table; `ZSTD_MatchState_t::hashTable`, `hashTable3`, `chainTable`, `tagTable` | Owned array/vector of positions, with explicit empty sentinel and checked history access | `unsafe-match-table`: unchecked table access after hash/index bounds proof | Each candidate is older than the current position and inside valid history; collisions require byte verification; stale entries are rejected. Compare insert/lookup cost and end-to-end ratio. |
| Checksum; `XXH64_state_t` | Four `u64` lanes, total length, `[u8; 32]` tail, and tail length | Shared value state; word access can use `unsafe-byte-cursor` | Tail length stays below one stripe; hash arithmetic wraps intentionally; frame checksum is the low 32 bits of XXH64 with seed zero. Compare stripe-heavy and short inputs. |
| Decoder/encoder context; `ZSTD_DCtx`, `ZSTD_CCtx` | Owned typed component fields, phase enums, options for reusable tables, and positions into buffers | Composes independently selected family backends; no raw context ownership | Reused tables belong to the active frame/dictionary; reset clears frame-local history; mutable components have unique borrows. Compare whole-frame calls, reuse, and allocation counts. |

The initial history implementation may retain complete output to simplify
one-shot decoding. That does not establish bounded-memory streaming; a later
ring buffer must retain only the permitted history and explicitly map logical
positions to physical indices.

## Structures required for broader upstream parity

Module 6 implements the streaming state, bounded ring history, shared sequence
sink, reusable block encoder, and incremental checksum described in
[streaming-design.md](streaming-design.md). These types own their allocations
and use indices for all resumptions. Ring access has the independent
`unsafe-ring-history` feature; scalar progress/phase/checksum state is shared.

Module 7 dictionary ownership, immutable prepared entropy, independent content
access, indexed two-region history, and prefix lifetime are specified in
[dictionary-design.md](dictionary-design.md). The remaining rows below describe
planned families; their features are reservations until implemented.

| Family and upstream counterpart | Safe ownership and indexing design | Unsafe comparison boundary and key obligations |
| --- | --- | --- |
| Prepared dictionaries; `ZSTD_CDict`, `ZSTD_DDict`, `ZSTD_prefixDict`, `ZSTD_localDict` | Borrow raw dictionary bytes for one call, or own `Box<[u8]>`; prepared object owns its entropy/match tables. Optional `Arc<PreparedDictionary>` sharing. Store content ranges instead of self-references. | Reuse `unsafe-fse`, `unsafe-huffman`, `unsafe-match-table`, and `unsafe-history`. Add `unsafe-dictionary` only for dictionary-specific access. All ranges must remain within the dictionary owner; dictionary/history crossing matches need two-region proofs. |
| Dictionary registry; `ZSTD_DDictHashSet` | Dictionary ID to owned/shared dictionary handle, using an alloc-compatible map or indexed registry | Future `unsafe-dictionary` lookup specialization only if measured. Never manufacture independent mutable aliases of shared prepared tables. |
| Workspace; `ZSTD_cwksp`, `ZSTD_customMem` | Separate typed owned scratch arrays initially. Later a caller-owned typed workspace with range handles and explicit reset generations. | Future `unsafe-workspace` may use aligned raw arena storage; it must prove size/alignment, initialized prefixes, non-overlapping live regions, lifetime, drop, and allocator identity. Safe code must not reinterpret `Vec<u8>` as arbitrarily aligned typed storage. |
| Optimal parser; `optState_t`, `ZSTD_match_t`, `ZSTD_optimal_t` | Initialized arrays of match candidates, frequency counts, prices, repeat-offset snapshots, and predecessor indices | Future `unsafe-optimal-parser` table access. Predecessor links are acyclic and in range; price arithmetic cannot overflow; invalid links are not dereferenced. |
| Long-distance matching; `ldmState_t`, `ldmEntry_t`, `ldmMatchCandidate_t` | Indexed buckets, rolling hashes, candidate positions, and explicit window epochs | Future `unsafe-long-distance` bucket access. Rebase or epoch invalidation must handle every retained position together; hash equality never establishes byte equality. |
| Block splitting and entropy snapshots; `ZSTD_blockSplitCtx`, `ZSTD_entropy*TablesMetadata_t` | Sequence ranges plus owned entropy snapshots; plain type and size records | Reuse relevant entropy/sequence backends; future specialized feature only for a distinct access algorithm. Chosen partitions cover each sequence exactly once. |
| Streaming state; `ZSTD_dStage`, `ZSTD_dStreamStage`, `ZSTD_cStreamStage` | Phase enums plus indexed input/output staging buffers and a bounded history buffer | Component features remain independent. Partial input, partial output, and resumptions preserve the exact phase; no saved borrow outlives a caller's buffer. |
| Worker queues; `ZSTDMT_CCtx`, thread pool and jobs | `std` adapter with owned jobs, immutable shared dictionaries, and explicit result ownership | No initial unsafe concurrency backend. If introduced, use a dedicated feature with synchronization, destruction, and `Send`/`Sync` proofs. Codec `no_std` support does not require threads. |

## Unsafe proof obligations

An `unsafe fn` documents every obligation in `# Safety`. For each operation,
consider allocation provenance, lifetime, initialized range, alignment, integer
overflow, aliasing, overlap, and exclusivity. State explicitly when unaligned
access is permitted. A nearby `// SAFETY:` comment must connect each obligation to
the preceding checks or an established private invariant. Calling an unsafe
helper from an unsafe function still requires a documented unsafe block.

Unchecked access must be unreachable from invalid public input. Debug assertions
are useful diagnostics but cannot be the only proof. Raw pointer addition is
itself subject to in-allocation bounds. Capacity is not initialized length.
`copy_nonoverlapping` is invalid for an overlapping LZ match; even a memmove-like
copy does not automatically implement repeated-pattern expansion, because later
bytes can depend on bytes produced earlier in the same match.

## Measurement contract

Run the same deterministic inputs in default mode, each individual unsafe feature,
and all unsafe features together. Record output bytes or decoded output equality,
error behavior, throughput, compressed size, allocation behavior, toolchain,
target, CPU, build flags, and workload seed. Keep checks outside timing where
possible, use a black box to prevent dead-code elimination, and report variance
over repeated runs. Safe and unsafe builds must use the same algorithm and
compression decisions. A slower unsafe result is a valid result, not evidence
that validation should be removed.

Use unit cases for empty and short slices, arithmetic boundaries, exact buffer
ends, invalid entropy tables, invalid distances, overlapping matches, and truncated
input. Run release tests as well as debug tests. Miri and fuzzing add evidence for
unsafe code when the relevant toolchain is available; absence of either is a
reported limitation, never a passing result.

Dictionary training ownership, bounded sample ranges, candidate heap ordering,
and the separate safe/unsafe frequency-table family are specified in
[dictionary-training.md](dictionary-training.md).

## Module 8: entropy encoder and bounded strategy structures

The following structures are planned before implementation in
[entropy-encoder-design.md](entropy-encoder-design.md). That document specifies
wire boundaries, publication rules, independent oracles, and ordered small
checkpoints. This section refines the initial inventory's grouped Huffman/FSE
rows: encoding codebooks and inverse FSE access receive their own independently
selectable features. The names below are reservations until their implementation
and verification checkpoints are complete.

| Structure and upstream counterpart | Ownership and safe representation | Independently selected unsafe access | Invariants and focused comparison |
| --- | --- | --- | --- |
| Extracted forward bit writer; `BIT_CStream_t`, the role of `HUF_CStream_t` | Own initialized byte `Vec`, `u64` reservoir, and pending-bit count; callers pass numeric fields and retain no output pointer | Existing `unsafe-bitstream`; initialization of reserved spare bytes followed by justified length growth | Fewer than eight pending bits between writes; fields at most 32 bits and values fit their width; checked reservation precedes mutation; one terminal marker and zero high padding. Extraction preserves existing encoded bytes before new uses are added. |
| Canonical encoding codebook; `HUF_CElt`, `HUF_CTableHeader`, `HUF_readCTable` | Own 256 initialized `Codeword { value, bits }` entries and a validated maximum length; `bits == 0` explicitly marks an absent symbol | New `unsafe-huffman-codebook`; decoder lookup remains separately gated by `unsafe-huffman` | Complete Kraft mass and ascending-weight/symbol assignment; reject missing symbols before encoding; new inline tables use at most eleven bits, prepared dictionary reuse may use twelve. Checked and unchecked lookups copy the same aligned initialized values from live immutable storage. |
| Paired literal table result; `HUF_writeCTable_wksp` plus the selected CTable | `BuiltLiteralTable` owns the actual codebook and its exact serialized description; sparse weight ranges use indices | Plain ownership record; its codebook and bit output use their own features | Description fallback changes both members together. Sparse holes remain zero; the last present weight is implicit. RLE/raw literals preserve prior tables. Full-alphabet dictionary training and its serialized output policy remain unchanged. |
| Standalone Huffman table and depth policy; `HUF_buildCTable_wksp`, `HUF_optimalTableLog`, `HUF_writeCTable_wksp` | Fixed indexed frequency/parent records and an owned exact description/codebook pair; scalar depth policy owns no source borrow | Reuses `unsafe-huffman-codebook`; policy and tree records share safe value representations | Explicit depth cap 1..=12, complete Kraft mass, sparse holes absent. Cost search compares representable descriptions under the cap; unrepresentable trees have an explicit no-table result. The standalone parser accepts twelve bits while inline builders remain limited to eleven. Weight encoding admits values 0..=12 without changing existing dictionary-training bytes. Fuzz requested, adjusted, heuristic and optimal depths remain distinct observations; independent Rust trees need not match C trees. |
| Emitted and pending entropy; `ZSTD_compressedBlockState_t`, previous/next entropy tables | Typed owned Huffman/LL/OF/ML tables and repeat triple; candidate replacements use owned values or reuse tags rather than self-references | Shared state/value records composing independently gated codebook/FSE storage | Only an accepted block publishes pending tables/repeats. Raw/whole-block RLE fallback preserves state; channels update independently; zero-sequence blocks have no FSE modes or state bits. Accepted bytes may be in the encoder-owned output queue awaiting caller drainage. Reset restores the selected dictionary/default frame state. |
| Generalized inverse FSE table; `FSE_CTable`, `FSE_CState_t`, `FSE_buildCTable_wksp` | Own initialized inverse-transition entries, initial states, symbol availability, and actual table log; checked symbol/state indices replace packed pointer arithmetic | New `unsafe-fse-encoder`; migrate existing inverse access out of `unsafe-fse`, which continues to control decoding-table access | Every present symbol covers every successor exactly once. Absent symbols are rejected before initial-state or transition lookup. Checked dimensions bound allocation and flattened indices. Explicit RLE has log zero; reused normalized single-symbol tables retain their nonzero log. LL/ML limits are log nine, OF log eight. |
| Forward repeat-offset planner; `rep[ZSTD_REP_NUM]`, `ZSTD_storeSeq`, sequence offset-value planning | Plain planned-sequence values and an independent three-offset candidate snapshot; literal/match source positions are checked indices | Shared scalar representation; chosen matches and entropy tables use their own backends | Plan in forward source order before reverse entropy encoding. Apply the zero-literal shift and positive rep0-minus-one case exactly. Duplicate repeats may produce different valid next triples. Check explicit `distance + 3` before conversion; publish a candidate triple only with its accepted compressed block. |
| Bounded candidate chain; `ZSTD_MatchState_t::chainTable`, `ZSTD_HcFindBestMatch` | Own bounded hash-head and initialized node arrays; nodes store source positions, predecessor indices, and reuse tags | New `unsafe-match-chain`, independent of the existing single-candidate `unsafe-match-table` | Links are empty or point to older initialized nodes with matching tags; recycled slots never authorize stale access. Hash collisions require byte checks. Validate index, tag, source lifetime, history interval, window, and dictionary eligibility. Fixed visit budgets prevent cycles or collision storms from creating unbounded searches. |
| Named strategy and lazy choice; `ZSTD_compressBlock_lazy_generic` parsing role | Plain strategy enum and tentative match/price records; one-byte lookahead retains no caller borrow after a call | Shared value records using the selected candidate/source backends | Expose only implemented `Greedy`, `Chain`, and `Lazy` policies with their documented budgets. No numeric upstream-level equivalence is claimed. Tentative lookahead cannot mutate committed repeats/entropy or make future positions eligible as earlier matches. |

All proposed core types use `no_std` plus `alloc` and explicit fallible allocation
boundaries compatible with the project's current Rust baseline. Plain `Codeword`,
transition, `Sequence`, strategy, and state-tag records require no artificial
unsafe representation. Each unsafe operation still needs a complete local
validity, bounds, alignment, initialization, aliasing, and lifetime proof; an
`unsafe fn` additionally needs the full caller contract in its `# Safety` docs.

The encoder retains at most `min(advertised_window, 128 KiB)` output history.
Candidate storage adds separately bounded search metadata, not permission to
read evicted bytes or retain all frame output. Prepared dictionary content and
entropy remain owned by the selected immutable shared handle. Frame positions
stay full-width logical counters independently of physical history/node indices.

Module 8 validation must compare each new feature alone, every existing feature,
and all features together, including safe decoding with unsafe encoding and the
reverse combination. Use identical deterministic workloads and decisions for
backend comparison. Record ratio, throughput, retained allocation, repeated-run
variance, malformed-input behavior, independent C decoding, and relevant Miri
results; a named strategy's different compression decisions are a separate
algorithm comparison.
