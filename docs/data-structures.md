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
| Caller-owned standalone entropy output; `HUF_compress1X_usingCTable`, `HUF_compress4X_usingCTable` | Temporary borrows of immutable codebook/input and mutable destination; reusable owned bit writer and four-stream vector; scalar stream-count/layout records | Composes existing codebook and bit-writer features; caller writes use safe slices | At most 256 KiB source. Compute exact sizes and native-word capacity slack with checked arithmetic before mutation. Inadequate low-level capacity, source partition, or quarter size returns no stream; invalid symbols remain errors. Copy to caller only after complete success, preserving canaries and destination on failure. Copying an already paired table description has its own exact-fit contract and does not claim C's capacity-dependent FSE/raw serializer policy. |
| Huffman target outcome and corpus totals; `tests/fuzz/huf_round_trip.c` | Test-only scalar parameter/outcome records, checked source-prefix borrows, owned guarded destination and reconstruction vectors; corpus aggregation stores counts and deterministic hashes | Shared safe harness composing all independently gated entropy backends | Preserve both BMI tail-consumption layouts, the original capacity range, and the full 256 KiB source limit. Distinguish short/RLE exits, unavailable table, insufficient description capacity, zero stream, and exact roundtrip. Record requested, adjusted, selected and actual depths separately; never turn a failed assertion or unsupported engine into a skipped corpus file. |
| Incremental sequence test source; `tests/seqgen.h::SEQ_stream`, `SEQ_outBuffer`, `tests/zstreamtest.c:1551` | Test-only generator owns a wrapping `u32` random state, saved state/byte, typed phase, remaining count and incremental seed-zero XXH64; generation borrows a caller slice and returns initialized length | Shared safe harness; the existing checksum uses safe values and byte reads | Preserve the exact random recurrence, high/low alphabet separation, guard bytes, saved-state replay and continuation across 1,024-byte calls. An unfinished request keeps the same kind/value. No input/output pointer is retained. Check lengths before writes; empty destinations do not advance bytes. The complete 971-request schedule produces 34,639,743 bytes without requiring their simultaneous retention. |
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

At the Module 8 checkpoint, the encoder retains at most
`min(advertised_window, 128 KiB)` output history. Module 11 extends this to the
selected horizon while keeping the same default; see its records below.
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

## Module 9: allocator-bearing ownership and platform boundaries

The following analysis precedes allocator implementation. The full allocation
inventory, toolchain evidence, and failure contracts are in
[allocator-design.md](allocator-design.md); exact upstream test obligations are
in [upstream-allocation-plan.md](upstream-allocation-plan.md). The unavailable
Rust 1.100 distribution is not a verified build. The installed 1.101 nightly
has compiled the standard `Allocator`/`Vec<T, A>` subset without feature gates;
the compiler checkpoint pinned `nightly-2026-10-08` with a conservative package
version floor of 1.101 before generic storage migration.

| Owner and upstream counterpart | Safe allocator-bearing ownership | Existing unsafe selection and invariants |
| --- | --- | --- |
| Allocator value; `ZSTD_customMem` allocation domain | Standard `A: Allocator`; aggregate construction may clone `A`, and non-Clone allocators can be supplied as external `&A` | Allocator choice is not an unsafe optimization feature. Each collection allocates and frees through its own retained value; distinct clones need not be interchangeable. No raw block transfer between allocator instances. |
| `History`; decompressed destination and sequence-copy role | `Vec<u8, A>` owns complete output; returning it moves the allocator with the bytes | `unsafe-history` retains release-mode bounds, overlap, initialization and capacity proofs. Failure before writes preserves valid output. |
| `RingHistory`; DStream window and encoder retained history | `Vec<u8, A>` owns the bounded initialized ring, with independent `u64` frame positions | `unsafe-ring-history` remains independent. Reserve before changing frame state; reset retains capacity and never authorizes evicted indices. |
| `BitWriter` and count writer; `BIT_CStream_t`, `FSE_writeNCount` | Allocator-bearing byte vectors and plain reservoirs; constructors receive an allocator explicitly | Existing `unsafe-bitstream` only. Complete reservation precedes pointers or length growth, and no pointer survives a possible reallocation. |
| `FseTable` and decoded weights; `FSE_DTable`, `FSE_readNCount` | Typed entry and scratch vectors own their allocators; inline RLE remains a value | Existing `unsafe-fse`. All entries are initialized before publication; fallible copies allocate fresh storage without changing the source. |
| `FseEncodingTable` and normalization; `FSE_CTable`, `FSE_buildCTable_wksp` | Transition, initial-state, normalized-count and remainder vectors include every temporary allocation | Existing `unsafe-fse-encoder`. Construction failure drops complete owners; no partial inverse relation becomes visible. |
| `HuffmanTable` and literal decoding; `HUF_DTable`, literals workspace | Decoding entries, weights and output buffers use typed `Vec<_, A>` | Existing `unsafe-huffman`. Allocator changes do not broaden inline/dictionary depth limits or four-stream source ranges. |
| Paired descriptions and codebooks; `HUF_CElt`, `HUF_writeCTable_wksp` | Description vectors carry allocators; the 256-entry codebook stays an inline plain value | Existing `unsafe-huffman-codebook`. Description and codes move together; validation scratch uses the chosen allocator too. |
| Huffman priority work; `HUF_buildTree` role | Replace the Global-only heap with checked typed storage or a justified fixed-capacity value representation | Preserve exact tuple ordering and tree output. Any distinct unchecked queue representation needs its own feature and comparison; no allocator-driven unsafe casts. |
| Literal and sequence contexts; previous/next compressed-block state | Every current/default/spare table, plan, symbol vector, stream and section owns its storage | Compose existing independent entropy backends. Move or swap entire owners on commit; failure/discard cannot publish partial entropy or detach allocators. |
| Match tables, chains and dictionary seeds; `ZSTD_MatchState_t` | Initialized position/head/node vectors use the selected allocator; seeds remain independent owned tables | Existing `unsafe-match-table` and `unsafe-match-chain`. Index/tag/source eligibility proofs survive movement and failed growth. |
| `BlockEncoder`, `Encoder`, `Decoder`; `ZSTD_CCtx`, `ZSTD_DCtx` | Allocation reaches staging, payload, literal, sequence, history and lazy strategy storage; caller buffers remain borrowed only during calls | Existing backend flags retain their contracts. A process error still poisons the context until reset; custom allocation does not promise rollback of accepted input or provisional output. |
| One-shot results; compress/decompress destination ownership | Construct `Vec<u8, A>` in its final allocator from the beginning; Global wrappers call the same generic algorithm | No copying or raw-parts conversion merely to change allocator types. Returned owners preserve allocator lifetime. |
| Dictionary payload and prepared tables; `ZSTD_CDict`, `ZSTD_DDict` | Serialized bytes, content ranges and prepared tables carry allocator ownership and validate before publication | Existing `unsafe-dictionary` content reads. Context and dictionary instances may differ; table copies and views must state allocator/lifetime requirements. |
| Shared dictionary handles; prepared-object ownership | Stable Global `Arc`/`Rc` can own allocator-aware payloads; any retained Global control block must be disclosed, with caller-owned views a separate contract | Do not use unstable custom Arc/Rc or invent reference counting silently. Owned/borrowed configuration must preserve persistent and single-use-prefix lifetimes. |
| Raw/formatted training; `ZDICT_trainFromBuffer` construction role | Sample windows, frequencies, priority candidates, selected ranges, serializers and final validation copies use explicit typed allocation | Existing `unsafe-training` remains specific to its access family. Replace the Global-only heap separately and preserve selection ties, dictionary bytes and IDs. Rust training still does not claim COVER/fastCover parity. |
| Allocation test ledger; upstream custom callback counters | A separate test helper records layouts, domains, attempts and live/peak bytes; per-instance failure injection avoids failing unrelated harness storage | Test `Allocator` implementations require complete safety proofs, including non-unwinding allocation/deallocation, valid alignment/lifetimes, non-overlap and failed-growth preservation. Balanced ownership and fail-at-N are added Rust assertions. |

Scalar frame metadata, repeat triples, codewords, sequence records, histograms,
tree nodes and checksum state keep their plain safe representations. Platform
checks distinguish compilation from execution, and test pointer-width conversion,
endianness, alignment and the no-pointer-atomics `Rc` path explicitly. Report
collection capacity separately from allocator layout accounting and any Global
control-block exception. Static workspaces, footprint estimators and automatic
shrinking remain separate upstream contracts.

Completed leaf checkpoints retain the existing Global constructors and algorithms:

- `History<A: Allocator = Global>` owns `Vec<u8, A>`, accepts owned or borrowed
  non-Clone allocators, and returns the same vector without conversion. Append
  and repetition reserve before individual pushes; the unsafe overlap path
  obtains pointers only after reservation. Failed growth preserves bytes,
  capacity, frame boundaries, and the exact allocator-bearing owner. Custom
  storage, distinct-domain swaps, failure recovery, and Global parity are
  verified in [allocator-validation.md](allocator-validation.md).
- `RingHistory<A: Allocator = Global>` retains its exact allocator without a
  Clone requirement. Window growth reserves before clearing old state; smaller
  windows, emission, wraparound, and reset reuse the same allocation. Owned and
  borrowed non-Clone cases preserve the existing logical-position and unsafe
  store invariants, including failed growth after wraparound.
- `BitWriter<A>` and `EntropyBits<A>` each retain one allocator-bearing byte
  vector without cloning it. Normalized-count serialization returns that vector
  directly. Reservation precedes reservoir/position changes and initialized
  pushes; failed writes preserve the complete old state. The const Global
  forward-writer constructor remains for existing const contexts, while its
  generic runtime constructor awaits those contexts' migration.
- `FseTable<A>` owns its decoding entries, with allocation-free inline RLE and
  fallible `try_clone_in<B>` into a separately owned allocator domain. Table
  construction/copy requires only Allocator. Weight decoding uses independent
  table/output owners with Allocator + Clone; a borrowed non-Clone allocator
  works without cloning the underlying value. Bounds and initialization proofs
  remain identical across `unsafe-fse` and safe lookup.
- `MatchTable<A>` retains one allocator-bearing position vector and requires no
  Clone. Initialization reserves before pushing the empty sentinel. Seed copies
  accept `MatchTable<B>` from an independent allocation domain, copy only the
  initialized indices, and retain the destination allocation. Lookup/reset and
  seed copying consume no further allocation requests.
- `MatchChain<A>` creates fresh heads/node owners from an allocator and one
  clone; clones may be distinct domains. Each typed vector retains the value
  that allocated it. `Candidates<'_, A>` borrows the complete chain without
  allocation or source-byte ownership. Seed copying accepts an independently
  allocated `MatchChain<B>` and preserves all checked slot/tag ordering.
- `HuffmanTable<A>` uses explicit owners for entries and weight scratch. A
  borrowed table and decoded `Vec<u8, B>` may use different allocator types;
  fallible table copies retain independent ownership. Literal replacement
  remains unpublished until all streams succeed. `BuiltLiteralTable<A>` and
  test-only `BuiltStandaloneTable<A>` retain their exact serialized allocation;
  validation borrows its allocator temporarily and drops scratch before moving
  the description. Inline canonical codes remain plain values.
- `FseEncodingTable<A>` owns transitions and initial states separately. It can
  invert a `FseTable<B>` without retaining a borrow or adopting storage. Inline
  RLE remains allocation-free. Normalized counts and ranking scratch use fresh
  owners; weight serialization returns the original requested allocator.
  `Selection<'_, A>` borrows current/predefined tables or owns a complete
  replacement with its description. Exact cost/tie decisions are unchanged;
  failed selection publishes nothing and drops all temporary owners.
- `MaxHeap<T, A>` owns initialized indexed values in a fallibly reserved vector.
  Huffman setup preserves `Reverse<(weight, minimum_symbol, node_index)>` ties.
  The queue uses checked indexing and swaps; it is bounded setup work, not a
  new unchecked access backend. Child arithmetic follows the `parent < len/2`
  guard and right-child bounds check. Tree nodes remain fixed inline arrays.
  Full/sparse description builders retain the requested output owner through
  fallback; standalone selection moves the entire winning candidate owner.

### Aggregate refinement before implementation

Table owners will expose private-field `FseTableView<'a>` and
`HuffmanTableView<'a>` records made of initialized shared slices and copied
metadata. Their lifetimes come from immutable owner borrows, with no retained
allocator reference or raw-pointer erasure. Checked lookup and existing unsafe
access backends live on the view; owner methods delegate to the same algorithm.
Inline FSE RLE remains distinct from normalized singleton tables with nonzero
state width. These Copy views have no owning allocation or additional unsafe
representation.

`Dictionary<A>` and `DictionaryEntropy<A>` retain bytes and prepared tables;
`DictionaryView<'a>` exposes validated slices and immutable entropy views
independently of the original allocator type. A shared `Arc`/`Rc<Dictionary<A>>`
continues to allocate its counts and entire inline object through Global. A
borrowed view allocates nothing and cannot outlive its owner. Reachable-capacity
metadata stays separate from ownership accounting. Constructors clone allocator
values only to create fresh owners; formatted parsing must include weight
scratch and all four tables before publication.

`SequenceState<A>` owns frame overrides, repeats, and an allocator used only for
fresh tables. Prepared entropy views are passed per call and never stored.
Reset drops overrides while retaining the allocator. Existing partial-error
semantics are preserved: earlier channel tables or repeats can already change
before a later parse/allocation/output error. The surrounding decoder must
remain poisoned until reset; this migration promises no transactional decode.

`LiteralEncoder<A>` owns its writer, stream bytes, and complete section with
fresh allocator values. Candidate building uses a fresh clone, while a selected
inline codebook remains a plain value. Writer and section allocation failures
cannot publish a candidate table. `SequenceEncoder<A>` retains default/current/
spare tables, symbols, plans, writer and section as complete owners. Dictionary
inverse tables are constructed before start-frame state changes; prepare errors
preserve committed tables/repeats and expose no partial section. Global const
constructors remain available; custom aggregate constructors run at runtime.

Raw training will reuse `MaxHeap<Candidate, A>` after a separate queue-only
checkpoint freezes its exact score-descending and sample/start/end-ascending
ordering. A checked peek borrows only an initialized root. Popping before a
stale-score reinsertion leaves a spare slot, so reinsertion needs no new block.
Sample windows, frequency cells, queued candidates and selected ranges each
own a fresh allocator value. Final content retains the original allocator and
moves directly into `Dictionary<A>`; borrowed samples never enter the result.
The frequency accessor keeps its existing independent `unsafe-training` flag.

Formatted finalization preserves the existing validation copy deliberately:
normalized-count descriptions, learned Huffman description, and assembled
serialization use fresh allocator owners; the original allocator is passed to
`Dictionary::from_bytes_with_mode_in` for final serialized ownership. Parsing
allocates fresh validated table owners before returning. Raw training content
is a separate temporary owner when finalization follows training. All metadata
and padding writes follow complete reservation; content IDs continue to hash
untrimmed/unpadded input. A future owned-byte parser optimization is separate
from this migration and must not bypass validation before publication.

Private block encoding retains an allocator factory plus independent owners for
literal bytes, logical sequences, payload, entropy contexts, lazy match tables,
dictionary seeds and bounded ring history. `BlockEncoder<A, D>` owns a sealed
dictionary handle; `MatchSource<'_, A>` borrows history and copies a temporary
validated dictionary view. No self-reference is stored. Output may use an
independent allocator type and never borrows the workspace allocator. Global
const construction remains available; generic construction occurs at runtime.

Frame setup can rebuild match seeds before a later inverse-table allocation
fails. Its private caller must reset and complete frame setup before reuse;
this migration does not promise rollback of every seed. Block failure can
change scratch, but reserved output emission precedes committed entropy
publication, and history commits only an accepted block. Reset invalidates the
active attachment and frame state. Copies use individual pushes after checked
reservation, preserving parser order, match eligibility, and emitted bytes.

The public streaming `Encoder<A, D>` adds staging and pending-output vectors,
each built with a fresh allocator value, around the generic block workspace.
The original allocator moves into that workspace's lazy-allocation factory;
a redundant stream-level factory is unnecessary. Shared or borrowed dictionary
attachments are independent of the allocator type. Input/output slices expire
at each process call, while a borrowed dictionary retains its explicit lifetime.
Allocator-last constructors validate options before creating storage. Reset
retains workspace/buffer owners, and existing prefix claim, pledge, directive
retry, provisional output, and poisoned-until-reset behavior remain unchanged.

Allocator benchmarking uses only ordinary safe source/caller buffers plus the
separate audited tracking allocator. Eleven deterministic codec workloads use
Global, owned tracking values with distinct clones, and borrowed non-Clone
tracking values. Global payload counters remain uninstrumented/null. Fresh
metric surveys record cold, reset-retained and final-release snapshots; peaks
are cumulative high-water values, not subtracted deltas. Dictionary reachable
capacity is separate from domain-owned live bytes. Ledger/setup and caller
storage stay outside payload accounting.

Streaming timing reuses contexts and includes reset/process; whole-buffer,
parsing and training timing includes fresh allocation and result destruction.
Full byte/fingerprint checks and independent metric surveys occur outside timed
regions. Five samples of at least 150 ms use identical inputs across backend
features. Tracking wraps Global, so its timing includes ledger, locking and
clone bookkeeping and is not a comparison with another heap implementation.
No timing starts until concurrent verification work has finished.


## Module 10: compression parameters and context lifecycle

Design precedes implementation; see
[compression-parameters-design.md](compression-parameters-design.md) for source
anchors, exact semantics and the distinction between inspection and executable
compression profiles. Module 10 introduces no heap-owning or pointer-bearing
structure and changes no existing unsafe access family.

| Data structure / upstream counterpart | Ownership and representation | Invariants and backend policy |
| --- | --- | --- |
| `SourceSize`; source-size hints and `ZSTD_CONTENTSIZE_UNKNOWN` | `Unknown` or `Known(u64)` plain enum | Known empty and unknown differ; reject the reserved sentinel as known; use checked ordinary size arithmetic and explicit unknown-plus-dictionary tier algebra. Shared safe representation. |
| `ReferenceStrategy`; `ZSTD_strategy` | Nine plain enum values distinct from the existing independent `CompressionStrategy` policies | A queried reference strategy is never silently admitted to an unsupported parser. Shared safe representation, no artificial unsafe operations. |
| `CompressionParameterRequest`; requested `ZSTD_CCtx_params` fields | Private level/override/hint values; automatic values represented explicitly | Validate before mutation; explicit overrides survive level changes; getters return requests; resolution never mutates requests or allocates codec storage. |
| `RawCompressionParameters`; unvalidated `ZSTD_compressionParameters` boundary | Seven public `u32` values, including a numeric strategy | Accept arbitrary input as data; concrete validation rejects invalid fields, while explicit adjustment applies the pinned signed-32 clamp before size adjustment. Never use this record as proof of validated settings. Shared safe representation. |
| `ResolvedCompressionParameters`; `ZSTD_compressionParameters` | Private concrete scalar fields with checked construction | Respect target-width bounds and source/dictionary adjustments; keep arbitrary raw adjustment inputs outside validated records; typed checked lookup for preset tables. |
| Pinned level presets; `ZSTD_defaultCParameters` | Attributed immutable four-tier, 23-row table | Exact source constants and source hash; negative acceleration is clamped before negation; selection is separate from adjustment and codec resource admission. |
| `ReferenceFrameParameters`; `ZSTD_frameParameters` | Plain flags | C query defaults remain distinct from existing Rust checksum-enabled encoder defaults. |
| Encoder configuration/reset state; requested/applied parameters and reset directives | Existing `Encoder<A, D>` retains its buffers and handles; new operations manipulate only checked values and complete owners | Active settings remain immutable; rejected configuration is non-poisoning; parameter reset releases dictionary handles from outer and workspace state; parameter-only reset preserves Finished until explicit session reset. Allocation retention is not automatic shrinking. |

Future configurable hash/chain tables, persistent search coordinates, binary
trees, optimal-parser predecessor arrays, long-distance buckets, stable borrowed
sessions and worker queues require separate design checkpoints. This scalar
foundation does not extend their current storage or ownership promises.

## Module 11: configurable named match search

The approved implementation plan is in `match-search-design.md`. It extends the
existing Greedy, Chain and Lazy consumers; reference profile values remain
separate inspection records. Default bucket/node sizes, key width, search effort,
128 KiB retention and encoded bytes are compatibility inputs captured before
implementation. New controls are exposed only after their consumers exist.

| Structure / reference counterpart | Ownership and safe representation | Equivalent unsafe boundary and invariants |
| --- | --- | --- |
| `MatchSearchOptions`; supported role of `ZSTD_CCtx_params` | Private concrete scalar fields: hash/chain logs, key width, separate Chain/Lazy visit budgets, output history limit. Invalid setter input leaves the complete value unchanged. | Shared safe representation; no pointer/owned storage or artificial unsafe feature. Operational ranges differ from reference parameter bounds. |
| `SearchKeyLayout`, `MatchKey`; `ZSTD_hash4/5/6/7/8` | Copied validated log/width descriptor and initialized little-endian key value carrying its width. Read exactly the selected initialized bytes; incomplete valid tails return None. Checked wrapping hash arithmetic is shared. | No unchecked input load. Key width and live layout must agree before a bucket is used; no zero-padding of missing logical input. |
| `SearchShape`; storage roles of `ZSTD_MatchState_t` | Immutable scalar dimensions, effective history, effort and checked requested-payload arithmetic. Includes all cached families and actual `size_of`/alignment. | Shared safe values. Requested payload is distinct from retained capacity, allocator growth peak, dictionary ownership and total codec storage. |
| `MatchTable<A>`; hash table role | One exact retained `Vec<usize,A>` owner, active initialized bucket count and key layout. Non-owning positions remain indices. Reconfiguration reserves before clearing/publishing logical state. | `unsafe-match-table` uses the same key compatibility and runtime index < initialized length guards as safe access. Valid allocation, alignment, initialization and exclusive borrow must be proved locally; capacity alone is insufficient. |
| `MatchChain<A>`; hash/chain roles | Two independent exact vector owners for heads and tagged cyclic nodes. Positions stay u64; slots/tags, newest-first ordering and examined-link budget remain checked. A borrowed iterator excludes reshape/reset. | `unsafe-match-chain` retains runtime initialized-prefix checks and tag validation. Seed copy requires equal key layout/head shape and seed nodes <= working nodes before mutation. Multi-owner reserve failure preserves logical state but may grow earlier capacities. |
| `BlockEncoder<A,D>` / `Encoder<A,D>`; context and dictionary match state | Seven potential cached search vectors across working/seed families and history. Attachments retain independent typed D ownership; no pointer into dictionary/input is stored by a table. A frame resolves options before allocation and rebuilds compatible seeds. | Existing family flags remain independently selected. Partial preparation failure follows process poison/reset semantics. Exact A owners stay paired with every allocation through grow/shrink/regrow; clones are not assumed interchangeable. |
| `MatchSource` and `RingHistory<A>`; window/prefix roles | Temporary immutable dictionary/history/input view with checked usize virtual coordinates and separate u64 frame positions. Ring retention is min(request, wire window), at most 1 MiB; default remains 128 KiB. | Existing dictionary/ring flags and lifetime checks apply. Width-aware dictionary/history crossings require complete keys. Ring commits only after successful block publication; source and dictionary distance eligibility remain distinct. |

Public initial limits are hash log 10..18, chain log 10..19, key widths 4..8,
Chain/Lazy per-probe visits 1..64, and history 1 byte..1 MiB. Greedy still probes
one candidate. All seven cached search vectors request at most 31 MiB on the
current 64-bit Link/Node layout, within the additional 32 MiB logical admission
ceiling. Use actual layout calculations on each target. Allocator over-allocation,
transient growth, other codec scratch and dictionary payload are not covered by
that requested-size ceiling. Empty one-shot frames can resolve zero effective
history without reserving a ring.

Chain dictionary seeding with content length D, key width W and seed nodes S
uses D.saturating_sub(S + W - 1)..=D - W when D >= W. Both bounds are width-aware;
there are exactly min(S, D - W + 1) starts. Greedy keeps its existing separate
bounded dictionary-distance suffix, including its documented potentially large
preparation cost. Dictionary-to-frame keys require retained frame byte zero.
Ordinary output matches obey the wire window; a smaller retention setting does
not cap matches within the current input block or change dictionary rules.

Configuration replacement remains allocation-free at the fresh pre-header
boundary. Session/frame-option resets preserve search settings; parameter/full
resets restore both option records and clear seeds/attachments without replacing
owners. Deferred preparation can fail and poison processing, and resetting must
recover it. Tests must exercise real storage/search effects, malformed/mismatched
keys, every representative allocation failure, default byte fingerprints and
pinned C decoding. Compare safe and unsafe implementations on identical default
and tuned deterministic workloads, reporting memory and search-cost differences
separately from accessor timings.


## Module 12: bounded reference Fast profile execution

Design: [reference Fast execution](reference-fast-design.md). This opt-in
whole-buffer path reuses pinned scalar resolution and implements admitted Fast
profiles; it does not rename independent Greedy or alter named-policy defaults.
Source borrowing, lookup/store ordering and explicit wire-repeat intent are
specified before implementation. All codec components remain no_std plus alloc.

| Structure | Ownership and invariants | Upstream counterpart / backend |
| --- | --- | --- |
| `FastExecutionPlan` | Shared plain value record. Resolve known input size before allocation; admit Fast, window logs 10–20, hash logs 6–18, selected keys 4–7, checked block/window/step/resource bounds. Unsupported strategies and resources fail explicitly. Chain/search scalars remain valid but have no invented Fast consumer. | `ZSTD_compressionParameters` and Fast literal/minimum-gain policies; no artificial unsafe variant. |
| `FastMatchState<A>` | Own one existing `MatchTable<A>` with its exact allocator. Persist sampled entries across blocks, clear once per frame. Whole-source positions are checked `usize` indices distinct from EMPTY; wider counters require checked conversion. No dense history reseeding or duplicate retained-output ring. | `ZSTD_MatchState_t` hash/window role; reuse safe / `unsafe-match-table` access with independently checked lookup and store. |
| `BorrowedFastInput<'a>` | Borrow initialized original input and a checked current-block range. Validate every key/match extent, block-end-derived low limit, source position and distance before access. Own no bytes; returned output keeps no borrow. | Contiguous no-dictionary prefix, replacing pointers with indices; safe borrowed representation. |
| `FastParseCursor` | Plain anchor, candidate positions/buckets, step and next-step boundary. Checked progress, exact sampling/update order and eight-byte reference search margin even for narrower selected keys. | `ZSTD_compressBlock_fast_noDict_generic` search pipeline; no separate unsafe parser. |
| Explicit offset intent / checked sequence plan | Plain explicit-distance or repeat-code choice. Validate literal-zero semantics, exact selected wire value and its next triple. Preserve existing canonical named planning. | `ZSTD_storeSeq` offBase; share safe validated records. |
| Pending Fast block state | Reuse allocator-owned literal/sequence/entropy buffers without invoking named search preparation. `SequenceEncoder` owns the authoritative committed wire-repeat triple; only accepted compressed output publishes pending entropy tables, literal tree and repeats. Raw/RLE discard those candidates. Sampled hash updates for initialized emitted source have a separate lifetime. | `nextCBlock` versus `prevCBlock`; existing backend families retain their own safety contracts. |

Reference Fast mutates only its first two parser repeat cells. Capture those raw
observations separately from the full triple reconstructed by replaying actual
wire offset values; do not assert that its untouched third cell is decoder state.
The first milestone uses caller-owned whole-input indices; future streaming
coordinates beyond `usize` require their own conversion/rescaling design.

Allocation admission precedes any output/workspace request. Output retains its
original allocator and every scratch vector its exact owner; owner cloning is
explicit. On failure, drop all unreturned output and scratch without changing
input or request. Compare every observed allocation-failure ordinal, safe and
unsafe table access, named default frames, and actual C parser/frame observations
before publishing numeric compression APIs.


Module 12's [offset interface](sequence-offset-intent-design.md) uses plain
`OffsetIntent` / `ResolvedOffset` values and `SequenceWithOffset` records.
Repeat variants denote wire values, whose distance depends on whether literal
length is zero. One Fast parser vector owns each sequence and its intent;
existing named sequence records, vector sizes and owner counts stay unchanged.
The sequence context reuses its current allocations and introduces no additional
owner or committed-repeat field. Its snapshot is a value-consistency check,
not a token proving frame identity or source eligibility.

The [oracle metadata](reference-fast-oracle-design.md) consists of owned fixture
bytes and checked scalar/range records. C pointer addresses, padding bytes and
uninitialized delimiter fields are never serialized. Actual sequence/state
observations are distinct from chronological instruction traces; collector
candidate publication is distinct from ordinary frame selection. These test
records need no artificial unsafe Rust representation.


`BlockEntropyEncoder<A>` is the planned shared composite of existing literal,
sequence and payload owners. It introduces neither a new backend nor another
allocator factory: safe/unsafe access stays in each existing child family.
Construction preserves existing child creation/clone order, and extraction
moves complete owners rather than adopting buffers through allocator clones.
The named parser keeps all current source/search owners. The whole-input Fast
supervisor owns only its required sampled table and intent-bearing parser
buffers; no unused ring or seed storage is constructed. A pure Fast header and
block-selection record carries checked lengths and flags with shared safe
representation. See the emission/ownership contract in the Fast design.

The implemented entropy aggregate is assembled with `from_parts` after the named
workspace constructs its children in the original interleaved clone order.
It adds no allocator factory or storage owner. Grouping fields changes ordinary
destruction interleaving, while each child still releases through its exact
allocator; no old field-drop-order guarantee is introduced. The neutral
extraction retains named scratch validity rules until the explicit Fast
preparation contract is integrated.

`FastFrameHeader` is a plain fixed-array record with at most fourteen
initialized bytes: standard magic, descriptor, optional power-of-two window
byte and known content size. It owns no allocation or source reference and
shares one entirely safe representation. Its input is the admitted Fast plan;
checksum and dictionary ID remain absent. Single-segment encoding requires
actual input size no greater than the selected window. The C-compatible known
size transition to an eight-byte field occurs at 0xffffffff, while the existing
named convenience header remains unchanged. The complete raw-frame bound uses
checked source/header/block-header arithmetic, with one final empty raw block
for an empty source and no fabricated huge input allocation in boundary tests.

Fast literal selection uses a plain private policy value shared by all backends;
there is no additional buffer or unsafe representation. Positive target length
selects raw bytes before RLE/reuse/frequency work. Otherwise Huffman admission
compares complete stream/jump/tree payload, excluding its header, against the
strict Fast gain boundary. Existing named selection and its owner/reserve order
remain unchanged. An available Rust codebook does not invent C's separate
HUF repeat-valid flag or its minimum attempted-input threshold.

The shared Fast block-preparation method owns no extra context field. Before
fallible work it discards candidates, clears stale payload, validates the supplied
committed-repeat snapshot, checks complete LL/ML reconstruction and exact offset
intent, then selects literal/sequence sections. Every failure and nonbeneficial
candidate clears exposed payload and pending publication without changing
committed tables/tree/repeats. Sampled match history and source/window eligibility
remain the parser's responsibility. Checked raw output reservation precedes the
later infallible append and commit; preparation alone publishes nothing.

The direct-oracle Rust test adapter uses a versioned binary value stream derived
only from frozen JSON observations and deterministic recipes. A checked reader
borrows the initialized static byte slice, bounds every scalar/count/range and
requires exact end-of-input. Each decoded case borrows its source/identifier and
owns only test metadata vectors for seeds, sequences and sorted table changes.
No C memory, pointers, native padding or uninitialized values are serialized.
These test-only records and their safe reader share one representation; they are
not a performance-sensitive codec data structure and need no unsafe feature.
The converter revalidates the frozen collection before generating or checking
the binary; normal Cargo tests need no C/Python process or new crate dependency.

The single-block parser concretizes the borrowed-input role as `FastBlock<'a>`:
a checked source slice, block range, retained prefix floor, layout, power-of-two
window and initial paired step. `parse_block` borrows independently owned table,
literal and intent-bearing sequence vectors without cloning any allocator.
Before sampling it checks repeat/layout compatibility and reserves both output
bounds while preserving old logical contents on reservation failure. Success
includes trailing literals in the output vector and returns a plain
`FastParseResult` containing the trailing count and separate raw-parser/full-wire
candidate triples. No committed repeat state is stored. Later internal errors
may leave scratch/table changes; the enclosing one-shot caller discards them.
The direct parser admits blocks of at least seven bytes; ordinary tiny blocks
are handled by the future frame wrapper. The parser itself forbids unsafe code
and relies on the already independently gated sampled MatchTable accessors.

The ordinary-frame trajectory adapter separately packs 23 frozen cases and 64
nonempty blocks, preserving zero-size source calls and tiny stale-candidate
markers. Its safe test reader regenerates each bounded source once, verifies
captured fingerprints, replays literal/match bytes and full wire publication,
and validates every sorted table delta and complete table fingerprint. Raw C
repeat cells remain separate from full wire semantics. Its scalar/range records
and owned test vectors need no unsafe backend; source, sequence and table counts
are checked before fallible reservation. Normal Cargo tests consume the derived
binary without JSON/gzip dependencies or processes. This adapter excludes the
partial headers and is not a compressed-frame decoder.

`FastMatchState<'a, A>` will bind one borrowed whole source to an admitted plan
and exactly three existing owners: the sampled table, literal vector and intent
vector. Its cursor counts successfully parsed source, not published wire state.
The retained prefix floor advances from the block start; the parser independently
derives the stronger ordinary-match floor from the block end. Tiny blocks advance
source position without replacing stale scratch or modifying the table. Empty
continuations change nothing. A parser failure poisons this private one-shot
state, because scratch and sampled cells may have changed. Argument rejection
before parsing preserves usability. Each next call receives the enclosing
entropy context's committed wire triple; no duplicate committed triple exists.
Construction moves complete owners and resets the table once for the borrowed
source. This composite adds no unsafe representation; table access retains its
existing individual feature. The later frame emitter requests the next block
only after publishing the previous block and drops both owners on any error.

`FastEncoderWorkspace<'a, A>` will compose that sampled state with the existing
`BlockEntropyEncoder<A>` and an emitted-source cursor plus failure/final flags.
The original allocator belongs to the returned output. After admission and the
complete raw-frame reservation, fresh clones construct the table, literal and
intent vectors, literal encoder and sequence encoder; the final workspace
factory moves into the payload vector. No named tables, chain, source copy or
ring are built. Empty input needs only its header and final empty raw block.
For nonempty blocks the wrapper parses before selecting raw/RLE/compressed,
uses the committed entropy repeat triple, and reserves the complete selected
block before appending. Only compressed emission commits candidate entropy;
raw/RLE emission retains search writes but discards candidates. Tiny blocks
bypass entropy and RLE, and nonfirst uniform blocks use the reference's
effective candidate-size-below-25 RLE gate. Internal failures invalidate the
private wrapper; preflight argument errors leave it usable. These composite
roles introduce no unsafe backend beyond their individually gated children.

## Module 13: bounded no-dictionary DoubleFast

The [reviewed design](reference-double-fast-design.md) pins the exact upstream
loop and resource scope. The private long-key descriptor and allocation-free
DoubleFastExecutionPlan, borrowed block/cursor records and independent table pair
and single-block parser and persistent search state are implemented and verified;
shared plain framing, scalar entropy policy and static dispatch are also
implemented. Private complete DoubleFast framing and bounded allocation-failure
tests pass. Public request execution resolves actual known input and dictionary
size zero once, then selects an admitted Fast or DoubleFast plan. Strategy
selection precedes algorithm-specific resource checks; output-bound checks
precede allocator clones or requests. Independent C, platform and performance
evidence is qualified separately in the validation report.

| Structure | Ownership and invariants | Reference counterpart / unsafe representation |
| --- | --- | --- |
| `DoubleFastExecutionPlan` | Copy values: checked source domain, resolved profile, window10..21, block limit, long/short layouts and each/summed allocation size. No owner or hidden clamping. | Resolved DoubleFast compression parameters and dispatch; shared safe values. |
| `DoubleFastBlock<'a>` | Borrows one immutable complete source with checked block range and retained prefix floor. All candidate/current/next positions are indices. | No-dictionary parser source/base/end/prefix pointers; no pointer-backed alternate required. |
| Long `MatchTable<A>` | Owns initialized usize cells indexed by eight-byte hashes at log6..18. EMPTY remains distinct from valid source index zero. Validate layout and initialized access before reads/writes. | `hashTable`, sized by hashLog; existing safe and `unsafe-match-table` implementations. |
| Short `MatchTable<A>` | A separate complete owner of initialized usize cells, log6..18 and key width max(4,minMatch). It contains positions, not linked predecessors. | `chainTable` in DoubleFast, sized by chainLog; same existing independently gated table type. |
| `DoubleFastTables<L,S>` | Composite of the two complete MatchTable owners, accepting independently owned allocator values without requiring Clone or allocator equivalence. Validate both layouts before either allocation; failed second construction drops the first through its exact owner. Reset both initialized tables without allocation. | The two noDict arrays and their workspace lifetime; composite introduces no unchecked access, and both leaves use the existing unsafe-match-table feature. |
| Parser cursor/result | Plain checked positions, cached buckets/candidates, step/threshold, two saved repeats, trailing-literal count and separate raw-parser/full-wire triples. | Local no-dictionary DoubleFast state; shared safe values. |
| `DoubleFastMatchState<'a,A>` | Moves both complete table owners plus separately allocated literal and intent-bearing sequence vectors; borrows source and tracks parsed position/floor/failure. Reset tables once per source. | Match state plus sequence store, with explicit Rust ownership; composite needs no new unsafe flag. |
| `ReferenceFrameSettings` / `ReferenceFrameHeader` | Checked plain known content size, window10..21/log and block limit; no allocation or algorithm admission. Header owns fourteen initialized bytes plus used length. Scalar output-bound overflow is rejected before allocating output. | Shared standard frame metadata; safe values only. Fast still admits at most window20. |
| `ReferenceSearchPlan` / `ReferenceSearchState` / `ReferenceEncoderWorkspace` | Static enums select one admitted plan and one complete search owner; inactive storage is never allocated. The shared frame workspace owns one entropy child, emitted position and poison/final flags. Original allocator belongs to returned output; fresh clones own workspace children. | Block-compressor selection and frame loop; safe composite. |
| Existing entropy owner | One literal/FSE/bitstream/payload aggregate with committed and candidate publication state. DoubleFast uses normal literals even with positive targetLength. | Block entropy state and minimum-gain policy; existing individually gated child implementations. |

The shared `reference_policy` module computes automatic raw-only literals from
the actual resolved strategy and target: only Fast with a positive target sets
the flag. It also supplies the strict gain `(size >> 6) + 2` used by admitted
Fast/DoubleFast literal and complete-block selection. The explicit flag crosses
`prepare_reference` / `select_reference`; no search plan or inactive parser is
owned by entropy storage. Named literal selection retains its distinct policy.
These scalar functions need no unsafe alternate or new allocator owner.

The version2 admission fixture keeps immutable historical and successor columns
for81 C selector records,92 interop inputs and43 original fuzzer-loop levels.
The offline adapter verifies fixed inputs before import and owns only bounded
plain records. C preset strategy identity is distinct from known-source-adjusted
tuples; archived interop tuples remain explicitly historical Rust observations.
No pointer representation or unsafe alternate is needed for these test records.
Six additive C obligations remain separate from their formerly unexecuted cases.

The broader public-frame oracle plan uses a separate bounded packet version.
A Rust adapter owns its initialized input/output vectors and borrows checked
source/frame ranges; its nine-field request is a plain record. The source-only
recipe generator owns immutable bytes and tuples, with prior source identities
checked before importing generators. No private C table or pointer is exposed.
The C helper has three complete guarded buffer owners with a conservative
13,631,872-byte aggregate payload/guard limit, excluding opaque codec workspace.
All helpers remain std/process test infrastructure, outside the alloc-only core.
The [protocol review](reference-double-fast-interop-protocol.md) specifies bounds
before implementation and execution; no new unsafe Rust structure is required.


Ordinary current-long/current-short candidates accept indices equal to the
block-end floor, because `ZSTD_selectAddr` uses `>=`. The later-long candidate
uses an explicit strict `>` condition. Backward extension may finish at the
floor after a checked decrement. Saved repeat eligibility uses the separate
current-position/retained-prefix bounds. All probes also need a positive valid
distance and enough initialized source bytes; a matching hash proves none of
those conditions by itself.

Long/short lookup and store order is explicit. Preserve cached current-long
state, short lookup before current writes, the later-long read after those
writes, the step<4 deferred long store and collision-sensitive complementary
writes. Source/index arithmetic is checked. Seven/eight-byte inputs and some
nine-byte inputs still refresh literal/repeat scratch without hashing; only
ordinary-frame sizes below seven take the stale-scratch bypass.

Both output-vector bounds are reserved before the first parser table mutation.
A failed second reserve may retain increased first-vector capacity, but logical
contents and tables remain unchanged. Later parser errors poison persistent
state. Raw/RLE retains both sampled tables and discards candidate repeats and
entropy; compressed publication follows complete output reservation/append.
There is no second committed-repeat copy in the workspace.

The long-key layout extension preserves existing Fast and named validation.
Each table checks `Layout::array::<usize>` and their checked sum; at both log18
maxima the table payload is 4 MiB on 64-bit or 2 MiB on 32-bit targets, excluding
all other allocations. DoubleFast's 2-MiB history cap does not widen Fast's
existing 1-MiB cap. A pair of existing table owners adds no new unsafe operation;
any future packed/pointer representation would require its own reviewed safety
contract, backend and feature instead of relying on this composite rationale.

The borrowed block's ordinary floor is max(retained floor, block end-window).
Its entry position skips one byte only when block start equals that floor;
repeat availability instead uses max(retained floor, entry position-window).
Those are separate checked scalar facts, not one interchangeable lower bound.
A copied search cursor retains current/next positions, positive step, next
256-byte growth threshold and the last complete eight-byte key start. It is
created only when both current and next keys fit the logical block end. Advancing
computes checked scalar positions and returns no cursor when the next pair no
longer fits; thresholds are never used as source addresses. Key extraction must
stop at the block end even if the borrowed source contains later blocks.
These shared safe records neither allocate nor form unchecked pointers.

The ordinary DoubleFast test adapter projects51 fixed source recipes and160
positive block observations into length-delimited little-endian records. It
borrows source/literal bytes and the original58 complete C frame files; bounded
vectors own schedules, sequences and the two independent sorted table deltas.
Both table maps start empty and are reconstructed with checked C-index bias,
key extents, bucket identity and full-map fingerprints. Candidate raw repeats
remain separate from full wire repeats and selected publication state. Empty
calls stay in the schedule; tiny calls carry no fresh candidate. The adapter
does not call the production parser to validate expectations. Full compressed
frame decoding is a separate test against the existing Rust decoder. Original
entropy/owner observations remain in the immutable NDJSON collection; partial
header-only probes are excluded from this complete-frame adapter. All records
use safe slices and plain values, with no new unsafe representation or runtime
dependency.

## Module 14: bounded no-dictionary reference Greedy

The [source review and design](reference-greedy-design.md) describes the next
implementation; the structures below are planned, not implemented. Resolved
window log above 14 selects row search, otherwise reference chain search.
Both are required before public Greedy admission. The scope is window log at
most 21 and hash log at most 19, preserving the separate existing Fast and
DoubleFast caps. The shared frame metadata already covers this window domain.

| Structure | Ownership and invariants | Reference counterpart / unsafe representation |
| --- | --- | --- |
| `GreedyExecutionPlan` | Copy values: actual-source-adjusted profile, selected branch, effective key width 4..6, checked layouts, search budget and output bound. No allocation or allocator clone during admission. | Applied parameters and `ZSTD_resolveRowMatchFinderMode`; safe values only. |
| `GreedyBlock<'a>` and parser cursor | Borrow one complete immutable source, with checked block range and retained prefix floor. Candidate floors are evaluated per probe. Chain and row have distinct eight/sixteen-byte search margins. | `ZSTD_window_t` and depth-zero `ZSTD_compressBlock_lazy_generic`; safe indices and ranges. |
| Reference chain index | Two independent complete `Vec<usize, A>` owners: hash heads and cyclic predecessor cells, initialized to an invalid sentinel. Slot translation accounts for C's index bias of two. Validate dimensions and summed payload before the first allocation; a failed second owner releases the first through its own allocator. | `hashTable`, `chainTable`, `nextToUpdate`, `ZSTD_HcFindBestMatch`; new `unsafe-reference-chain` leaf. The existing named `MatchChain` has different semantics. |
| Row match table | Independent initialized position and tag/head owners, `Vec<usize, A>` and `Vec<u8, A>`, each with `2^hashLog` cells. Each row has 16/32/64 cells; cell zero is reserved, and insertion cycles backward through the remaining cells. Snapshot candidates in cyclic tag order before inserting the probe. | `hashTable`, `tagTable`, `ZSTD_row_nextIndex`, `ZSTD_RowFindBestMatch`; new `unsafe-row-table` leaf. |
| Row layout and salted key | Copy values for row width, bucket, tag and salt; explicit little-endian hashing and checked width arithmetic. A new private layout admits hash log 19 without widening existing named/Fast/DoubleFast key descriptors. | `rowHashLog`, `ZSTD_hashPtrSalted`; shared safe values. |
| Candidate snapshot | At most 64 plain source positions, with checked count and initialized access. Expired candidates terminate collection; future/invalid positions never authorize a source read. | Local row `matchBuffer`; safe value storage, no artificial unsafe backend. |
| Greedy search state | Own only the selected complete table family and existing literal/intent vectors, borrow source, track next-to-update and poison state. Scalar hashes may replace the C cache only while preserving every insertion and search boundary. | Match state and sequence store; safe composite, independently gated leaves. |
| Shared reference frame state | Extend existing static dispatch with one Greedy variant; preserve original output ownership and independent entropy owner. Raw/RLE retains search writes but discards candidate repeats/entropy; only complete compressed append publishes them. | Existing reference frame loop and block-state transaction; no new unsafe composite. |

Chain search is bounded by `2^searchLog` with search log at most 8 and chain
log at most 18. Row search allocates no chain and caps visits at
`2^min(searchLog, rowLog)`, at most 64; unused chain capacity is not a reason
to reject that branch. Table payloads use Rust `usize` widths, not C `u32`
widths. Both layouts and their checked sum must precede allocator activity.
Moves, swaps, reset and failure keep the exact allocator with each owner.

Safe and unsafe leaves share checked public contracts, reject malformed indices
before byte access or mutation, and compare complete table maps under identical
workloads. Unsafe functions must document validity, bounds, initialization,
aliasing, lifetime and applicable alignment requirements; every unsafe block
must prove them. Scalar Rust rows require natural element alignment. The future
C oracle separately needs 64-byte-aligned hash storage and row-aligned tags for
its prefetch assertions; that oracle requirement does not justify changing
Rust owner alignment.

The fresh row salt is derived from a zero-initialized C context and its first
salt advance; freeze that observation before choosing the Rust constant.
Explicit dictionary salt zero and context-reset salt changes remain future
contracts. Candidate raw repeats and committed wire repeats remain separate.
The source-only admission forecast does not constitute executed Greedy tests
or completion of any pending upstream migration obligation.

The [direct oracle protocol](reference-greedy-oracle.md) constructs initial
chain/row maps from strictly increasing seed positions rather than accepting
arbitrary pointer-like cells. It records complete maps as zero-initialized
owners plus canonical before cells/deltas and full fingerprints. The std-only
helper owns guarded 64-byte-aligned arrays with checked aggregate costs; the
source arena includes initialized leading/suffix slack without extending logical
history. Fresh-context salt observation has a separately bounded custom C
allocator and does not share direct-parser table owners. These test records
and borrowed packet fields introduce no unsafe Rust representation.

The implemented `GreedyKeyLayout` admits H6..19 and validates minMatch 3..7
before selecting effective width 4..6. `RowLayout` selects 16/32/64 cells and
at most 64 visits without a chain allocation. Its salted `RowKey` retains the
layout identity; higher unused bytes never enter exact-width loads. These
records share safe scalar code across every backend. The previously captured
fresh-context salt is `0x835892b4bb2cae74`; reuse/dictionary salt behavior remains
outside this checkpoint. Primitive tests reuse 1,728 independent pinned C hash
vectors with algebraic salt high-bit expectations, plus all dimension bytes,
short tails, incompatible widths, reserved row geometry and maximum hash bits.

`ReferenceChain` now owns independent initialized head/link vectors. Insertion
validates both live indices and a strictly earlier replaced head before either
write. Its borrowed `ChainCandidates` yields a checked head first, then stops
following predecessors once the yielded candidate's distance reaches the ring
size. Each further link must descend strictly; future indices and cycles yield
one error and terminate. The caller still proves source-byte bounds. Slot
translation masks before adding the raw-index bias of two, avoiding overflow
near usize limits without claiming C large-index reduction equivalence. Reset,
move, swap and either allocation failure retain the exact owner values; no
allocator clone is required. Safe/all-feature focused tests cover these
contracts; parser/oracle full-map integration is a later work item.

The `unsafe-reference-chain` leaf selects checked unchecked-access operations
for the same head/link owners. Every initialized bound is checked at runtime
before access, and each local proof covers alignment, initialization, exact
ownership, aliasing and temporary-reference lifetime. No unsafe function is
exposed to callers. Six focused tests pass with safe, isolated-feature and
all-feature builds, including an independent unbounded source-history model for
sparse cyclic traversals; focused no_std and strict feature/all Clippy pass.
The feature is included in the normal every-feature/no_std matrix. No speedup
is claimed before the separately planned deterministic Greedy benchmarks.

The safe `RowTable` now owns initialized positions and independent tag/head
bytes with natural alignment. It checks the complete row against both live
owner lengths before access, validates the reserved head, and rejects invalid
or nonincreasing updates before either owner changes. Candidate collection
starts at the head, skips reserved cell zero, follows cyclic tag order and stops
on the first matching empty/expired position. A future position rejects the
state before any source access. `CandidateSnapshot` owns at most 64 copied
indices, so inserting the query cannot overwrite saved candidates. Six focused
safe/all-feature tests cover complete row maps and wrap for all widths, mixed
tags against a source-history model, zero tags, budgets/floors, malformed cells,
truncated owners, independent non-clone allocators, both allocation failures,
and moves/swaps/reset. Strict safe Clippy and no_std compilation pass.

The independent `unsafe-row-table` backend uses the same checked row and owner
contracts. Before unchecked reads/writes, runtime checks prove the entire row
fits both initialized owner lengths; head validation and power-of-two masks
bound every relative index. Local proofs cover both allocations, alignment,
initialized values, aliasing, overflow and the lifetime of every temporary
reference. Candidate snapshots continue to use only safe array access. All six
row tests pass in safe, isolated-feature and all-feature modes, including full
maps and the independent source-history model. Strict feature/all Clippy,
isolated-feature no_std and all fourteen unsafe features together without std
pass. The feature is included in the normal matrix; performance remains unmeasured.

`GreedyExecutionPlan` now resolves the actual known size once and selects rows
only for the resolved window above 14. Its copied `GreedyTablePlan` retains only
the selected family's dimensions and budget. Before any allocation or clone,
planning checks W21/H19, chain-only C18/S8, both independent target payload
layouts, their sum, and the complete raw-frame bound including the output's
isize allocation limit. Row mode never rejects an unused chain size or an
otherwise valid search log above six. Six safe/all tests reuse five frozen C
Greedy query tuples and check source-tier transitions, exact owner costs,
strategy/resource precedence, key clamping, target independence, empty hints
and nontruncated source sizes above u32 on 64-bit hosts. Strict safe Clippy and
no_std compilation pass. This private plan does not yet change public admission.

`GreedyBlock` now borrows the complete source with a checked nonempty block
range, retained floor and window. Key extraction stops at the logical block
end even if later source bytes exist. Candidate floors advance per probe;
retained prefix and entry-repeat floors are separate. `GreedyCursor` preserves
strict main-loop and inclusive cleanup limits with chain/row margins 8/16,
and derives miss steps from the distance to the literal anchor. `GreedyMiss`
retains the skipping flag even on loop exit. Five safe/all tests cover tiny
fresh/prefixed boundaries, moving floors, invalid ranges/counters, truncated
keys, anchor reset and the frozen no-match C schedules at 2047/2064/4096 bytes.
Strict safe Clippy and no_std checks pass.

The next direct-fixture projection is test-only: bounded little-endian records
retain copied scalar metadata and sparse complete table maps, while source,
name and literal slices borrow immutable fixture bytes. A checked safe reader
owns bounded metadata vectors, validates counts before reserving and replays
sequences/fingerprints without JSON, C bindings or subprocesses. These plain
fixture records need no unsafe backend. Projection and Rust reader are separate
work items so immutable provenance can be reviewed before parser execution.

The test-only `greedy_oracle_fixture` now reads the 173 projected cases with
bounded, fallible metadata allocations and borrowed source/name/literal spans.
Raw C frontiers are checked before removing the bias of two. Canonical complete
maps validate owner roles, ordering, byte widths, source bounds, fingerprints,
reserved row heads and hash/tag membership. Independent sequence replay checks
raw/expanded lengths, offset intents, long markers and separate raw/wire repeat
triples. Seeded Rust owners match all 346 C initial maps. Five tests pass on safe,
unsafe-reference-chain alone, unsafe-row-table alone and all features; strict
safe/all Clippy, formatting and no_std compilation pass. Truncation, dimensions,
counts, semantic forgeries and coherently rehashed bad heads/positions reject.
The reader is excluded from production builds; full Greedy parsing is pending.

The next search layer borrows one checked `GreedyBlock` and one exclusive table
owner; a copied `GreedyMatch` contains only an earlier source index and length.
The caller owns the next-to-update scalar. Chain search validates branch, key
identity, budget and probe coordinates before inserting pending indices, then
borrows the chain for bounded candidate traversal. Skipping inserts only the
first pending index. Byte comparison uses checked source slices and strictly
longer matches, preserving newest-candidate ties. No source pointers, new
allocations or unsafe composite are needed. A table corruption error after an
update leaves partial search writes; the future retained context must poison
itself instead of publishing/reusing that state.

Row search uses the same safe match record and source comparator. It borrows
the selected `RowTable` exclusively, with caller-owned update frontier and salt
entropy scalars. A gap above 384 inserts the first 96 and last 32 pending
positions; skipping mode inserts no pending positions. The query's salted hash
contributes wrapping entropy once. Candidates are copied before inserting the
query, so overwriting a row slot cannot change that search's candidate order.
Scalar hash computation replaces only C cache/prefetch machinery, preserving
the sixteen-byte logical tail and insertion schedule. No Rust cache-equivalence
claim is made; C cache fields remain preserved in the fixture evidence.

The direct parser uses a borrowed `GreedySearch` enum for static chain/row
dispatch; each variant retains the exact two allocator-parameterized owners by
exclusive reference. A copied `GreedyProgress` holds next-to-update, row salt
entropy and skipping. Caller-owned literal/sequence vectors reserve fallibly
before any logical mutation. Tentative source matches retain indices only;
repeat-first matching bypasses sampling, while explicit matches may extend back
to the literal anchor and retained prefix floor. Immediate repeat cleanup
changes neither sampling table. Raw parser repeats retain their separate saved
offset restoration, while emitted `OffsetIntent` values independently advance
complete wire repeats. All new records/composites are safe; they add no unsafe
representation beyond the existing selected leaf backends. Returned literals
include the trailing suffix; publication and failure poisoning belong to the
separately planned ordinary retained context.

Additional ordinary source review establishes two ordered frontier transitions:
the frame loop first clamps next-to-update to the retained window low limit for
every positive call, including tiny calls. Only a fresh length>=7 candidate then
applies the 384/192 long-gap adjustment. A zero call does neither. The direct
parser already consumes a prepared frontier and must not incorporate either
ordinary-wrapper transition. The retained state will track this scalar
separately from table mutation and candidate publication.

The ordinary Greedy fixture projection borrows names, source bytes, literals
and complete frame bytes from an immutable little-endian test file. Bounded
metadata vectors own schedules, positive-block observations, candidate sequences
and both table delta streams. Each copied sampling record preserves the raw C
frontier (bias two), skipping, row geometry, salt/entropy and eight cache values;
cache retention is evidence, not a Rust cache-equivalence requirement. Zero calls
remain in the schedule, with their no-change observations verified in the frozen
source collection. Positive blocks retain before/after sampling, window floor,
separate raw/wire repeats, raw sequence lengths and long markers. The checked
reader must reconstruct complete tables and replay source bytes independently
before production parsing uses any expected value. These test records and
borrowed spans add no unsafe operations or performance backend. Full entropy
observations remain in the source collection; exact complete frames cover later
encoder publication. Projection and reader have separate checked work items.
