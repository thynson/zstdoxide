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

The ordinary checked Rust adapter now borrows all source/literal/frame spans
and owns bounded fallible schedule, block, sequence and delta vectors. A
separate semantic replay reconstructs every complete table and validates
sampling continuity, ordered frontier adjustments, raw/expanded lengths, long
markers, source membership and selected repeat publication without calling the
Greedy parser. All 151 complete C frames decode to 13,587,614 expected source
bytes with the existing Rust decoder. Five focused tests pass in safe, isolated
chain, isolated row and all-feature builds, including coherently rehashed bad
positions, tiny-floor and cache/salt forgeries. Strict safe/all Clippy, formatting
and no_std compilation pass. No production encoder admission changes.

The ordinary retained `GreedyMatchState` owns exactly one selected
`GreedyTables` variant (chain heads/links or row positions/tags) plus separate
literal/sequence scratch vectors, all preserving their supplied allocator
values. It borrows one complete contiguous source and stores only logical
position/floor/progress scalars. Rebinding resets prior indices before sampling
another source. Empty calls preserve all state; tiny positive calls clamp the
frontier and advance the source/floor while retaining stale scratch and tables.
Fresh calls apply the separate long-gap adjustment before direct parsing. The
caller alone supplies committed wire repeats and chooses candidate publication;
raw/RLE selection never rolls back sampled tables. Invalid arguments precede
mutation. Any parser/allocation failure poisons the retained context, including
future empty calls, so partial writes cannot be reused. This safe composite
adds no new unsafe leaf and must be checked with both existing leaf switches.

All 395 ordinary C blocks now match the Rust retained context without reseeding:
367 fresh candidates, complete maps, literals/sequences, frontiers, skipping,
salt entropy and independently committed wire repeats. The 25 raw and 12 RLE
changed-repeat candidates remain discarded while sampled history survives.
Seven tests pass on safe, isolated chain, isolated row and all-feature builds.
They cover invalid arguments before requests, either table/scratch allocation
failure, exact independent non-clone owners and scratch reuse, and an injected
future candidate that rejects after a pending insertion and poisons all later
calls. Strict safe/all Clippy, formatting and safe/combined-leaf no_std pass.
The context does not compare Rust cache machinery with C cache observations.

`ReferenceSearchPlan` and `ReferenceSearchState` now include a private Greedy
variant, allocating only its selected two tables followed by literal/sequence
owners. Existing Fast/DoubleFast construction order and request admission are
unchanged. Five dispatch tests pass in safe/all builds, including exact owner
counts/bytes, source mismatch before clones, borrowed non-clone allocators and
ordinary mixed-fallback candidates. All nine frozen Fast/DoubleFast baseline
tests retain complete frame bytes and owner observations. Strict safe/all Clippy,
formatting and no_std pass; public Greedy requests still reject until v3 mapping.

The shared frame workspace now verifies private Greedy execution without adding
an owner or unsafe composite. Sampling state advances independently of entropy
publication; failed output append preserves the previous output and committed
repeats, clears tentative Huffman state, and poisons the workspace. Six focused
tests pass in safe, isolated chain, isolated row and all-feature builds. They
decode 274 Rust frames and reproduce the ordinary C schedule's 181 raw, 38 RLE
and 176 compressed choices, including 25/12 discarded repeat changes. Every one
of 6,184 observed whole-frame allocation ordinals is failed independently, with
all exact owners released. Borrowed non-clone allocators and invalid schedules
retain their contracts. Strict safe/all Clippy, formatting and safe/combined-leaf
no_std pass. Complete Rust entropy bytes are not claimed equal to C frames.

Public reference dispatch now resolves the actual known source once and admits
the Greedy plan after the separately frozen v3 policy. No owner layout, parser
algorithm or entropy supervisor changes are needed. Chain/row resource checks
still occur before allocator cloning or allocation. Test-only inspection helpers
replace temporary integration-wide dead-code allowances. Four public Greedy
tests cover both selected families, source hints, resource/error ordering,
equivalent settings and exact owned/borrowed allocator lifetimes.

The next public interoperability adapter owns three initialized C buffer bases
(source, compressed input, output), each with checked capacity and 64-byte guards
on both sides. Only live within-allocation pointers reach the public C APIs;
the context owns its opaque tables and is freed before buffer owners. The new
Rust adapter owns a bounded packet vector and returns owned frame/decoded vectors,
borrowing validated source/frame ranges by index. Python recipe records own
deterministic bytes and immutable scalar tuples; manifests retain historical
source/request identities separately from successor admission. A separately
versioned wire magic prevents old-policy transport confusion. Source/frame and
stream caps do not claim a total bound on arbitrary C workspace. The controller
will send C encode requests only from the frozen admitted set. Detailed bounds
and implementation work items are in `reference-greedy-interop-protocol.md` and
`TODO.md`; these tools remain outside the no_std codec and add no unsafe Rust.

The Module 14 benchmark harness is std-only and owns initialized source/frame
Vec values and separate allocator-survey snapshots. It adds no unsafe data
structure: chain and row accesses still use the independently gated codec
leaves. Frozen requests, source identities, ownership accounting and clock
boundaries precede implementation in `docs/reference-greedy-benchmarks.md`.
The full 32-case plan retains eight historical Fast/DoubleFast controls and
requires row-64/row-capped complete-frame and owner equivalence on each source.

Module 15 preparation analyzes shared Lazy/Lazy2 selection in
`docs/reference-lazy-design.md` before implementation. New records are copied
lookahead depth, candidate, checked gain and bounded query coordinates. They
reuse the exact two selected table owners and separate output scratch, adding
no owner or unsafe leaf. Strict main-search and inclusive speculative queries
need distinct checked contracts. Rejected speculative candidates still mutate
sampling, while frame publication retains the existing transactional boundary.

The versioned direct Lazy C oracle adds only a copied depth scalar to its
existing Recipe. All guarded owning arenas and initialized before-images keep
their exact predecessor contracts. The new scalar selects one of six unchanged
upstream entry points; it creates no new pointer relationship or owner. The
packet, inclusive lookahead arena proof and source review precede execution in
`docs/reference-lazy-oracle.md`.


The broader Lazy fixture retains a bounded immutable NDJSON byte pack with
consecutive byte extents and SHA256 identities for 645 direct groups and six
mode/depth salt groups. Its Python reader owns decoded metadata and bounded
record slices, reuses the independent complete-map/sequence replay validator,
and compares all tiny/depth-zero predecessor outputs. Score observations are
copied final sequence/frontier/fingerprint records over 96 pre-frozen inputs;
they classify publication outcomes, not chronological probe traces. These
std-only offline validation records add no codec owner or unsafe backend.
The future Rust projection remains a separate checked work item.


The direct Lazy Rust projection uses a separately versioned `ZLZST01` envelope:
645 length-delimited records, each with a copied depth byte (0..2), three zero
reserved bytes and the unchanged Greedy observation body schema. The depth
corresponds to the v1.5.7 lazy-family entry-point selector and direct packet field
at byte 144. Its name prefix must agree with depth; the selector is not inferred
from a table layout or final output. The 64-MiB collection/32-MiB record caps
bound reading before allocation. Name/source/literal spans will borrow immutable
fixture bytes; bounded seed, sequence and sparse-map vectors own initialized
metadata with fallible reservation. The common checked Greedy body reader and
independent sequence/table validators can be reused through test-only methods,
without changing the legacy pack or its strict search contracts. Neither copied
depth nor borrowed evidence spans introduce a performance leaf or unsafe variant.
Projection and the Rust adapter remain separate verifiable TODO items.


The checked `LazyCase` adapter now owns only a copied depth and the common
`OracleCase` metadata. It validates depth/reserved/name agreement before returning
borrowed records, with bounded fallible collection allocation. The common reader
and semantic validators are shared through test-only crate visibility; complete
value equality supports all 173 predecessor comparisons. Five focused tests
replay all 645 sources, verify every complete initial/final map and compare all
1,290 seeded Rust owners. Safe, isolated chain, isolated row and all-feature
builds pass; these fixture records add no unsafe implementation or codec owner.


The copied `LazyDepth`, `LazyCandidate` and `LazyGain` records now make the
selection contract concrete. Candidates retain a checked source position,
positive bounded distance, at least four matched bytes, intent and positive
offBase. Their signed checked gains preserve negative short/distant scores and
strict threshold ties. `LazyQuery` is produced by advancing a strict coordinate
once, so equality is admitted only through explicitly named speculative search
entry points. Those entry points revalidate the coordinate against their block
before writes; Greedy's existing main-search functions still reject equality.
All records are allocation-free safe values shared by both table backends.


Depth-one parsing now reuses the Greedy parser's existing scratch owners and
publication result through `parse_with_depth`. Baseline repeat candidates are
copied before mandatory table search; strict length ties retain the repeat.
The speculative loop advances its own query independently of the chosen start,
updates the incumbent with checked gains, and restarts only on an explicit
improvement. Rejected probes retain their sampling writes. Backward extension,
repeat cleanup and wire replay remain shared. The new direct entry point remains
private; ordinary contexts still select Greedy until their separate capture gate.


Depth two uses the same checked candidate and inclusive query records. It takes
one further probe only after the depth-one explicit proposal fails, weights the
repeat by four, and uses the seven-point explicit incumbent penalty. Either
explicit win restarts depth one at the next query; a repeat-only win terminates
selection after the eligible depth-two probe. The query may reach the inclusive
limit while the selected start remains earlier. No new owner or unchecked
operation is introduced; all 645 frozen depth-tagged observations exercise the
shared state transitions and complete final sampling maps.


The ordinary Lazy-family protocol is designed in
`docs/reference-lazy-frame-oracle.md` before C execution. Source-only `Recipe`
records add copied depth and optional predecessor identity to the immutable
Greedy ordinary input fields. Source byte values and schedules are owned Python
objects with the same fixed caps; no seeds, caller repeat overrides or predicted
outputs are admitted. The planned C helper extends only the parameter selector
and metadata while retaining the existing guarded arena, scratch, frame, replay
and before-map owners. Independent replay remains a separate bounded test-only
consumer. No production allocation or unsafe leaf is added by these records.


The planned ordinary Lazy Rust fixture uses a separate `ZLZFR01` envelope with
510 cases and 1,377 positive blocks. Each bounded case body begins with copied
depth 0..2 and three zero reserved bytes, followed by the unchanged Greedy
ordinary trajectory body. The outer cap is 256 MiB and each body is at most
32 MiB; existing per-source (4 MiB), per-block (128 KiB), schedule (64 calls) and
nested record caps stay unchanged. `LazyFrameCase` will own depth and common
`OracleFrameCase` metadata; source, literal and complete frame bytes borrow the
immutable pack. Sequence/schedule/sparse-map vectors retain fallible bounded
allocation. The shared reader gains only test-only crate visibility, and the
wrapper checks selector/name consistency, duplicate names, exact counts and
exhausted extents. Shared independent semantic/table/frame replay does not call
the production parser. Copied depth adds no unsafe backend or production owner.


The ordinary `LazyFrameCase` adapter now implements that envelope. Common
`OracleFrameCase::read`/`validate` gain test-only crate visibility and records gain
value equality without changing their decoding or replay bodies. Five tests
check all 510 depth-tagged observations, lossless equality of 138 predecessors,
549 complete C frame decodes, bounded malformed envelopes and mutated actual
Lazy/Lazy2 records. Immutable byte spans stay borrowed and metadata reservations
remain fallible. The codec and its public admission are unchanged by this reader.


Retained ordinary Lazy/Lazy2 state will extend the existing copied
`GreedyExecutionPlan` with a `LazyDepth`. A private family constructor admits
exactly Greedy/Lazy/Lazy2 and keeps the resolved strategy unchanged; the existing
Greedy-only constructor and public search admission remain strict until the
successor admission gate. Source-domain error precedence, W21/H19 and chain
C18/S8 resource limits, row selection, mathematical allocation layouts and frame
bounds remain shared. These scalar records need no artificial unsafe variant.

`GreedyMatchState` will dispatch the same borrowed block and four exact existing
owners (two selected tables, literals, sequences) through `parse_with_depth`.
No inactive table, allocator clone or new allocation is introduced. Zero calls,
tiny stale-scratch calls, window/frontier adjustment and poison-on-parser-error
remain common; speculative writes persist across raw/RLE fallback while only
caller-committed wire repeats enter the next block. This corresponds to the
shared v1.5.7 `ZSTD_buildSeqStore`/`ZSTD_compressBlock_lazy_generic` context path.
All 510 ordinary observations will exercise uninterrupted source ownership and
complete maps across schedules, with the 138 predecessor suite retained.


Private complete Lazy-family frame integration will reuse the existing
`ReferenceSearchPlan::Greedy` / `ReferenceSearchState::Greedy` storage variants
for all three depths. The legacy variant names describe shared ownership, not
public admission. No new enum owner or duplicated repeat/entropy state is needed.
The frame workspace alone owns committed entropy and repeats; candidate literals,
sequences and speculative search writes live in its selected search context.
Output reservation and append must succeed before candidate entropy/repeats
publish, and errors poison the workspace without advancing the emitted extent.
Tests will pass exact captured resolved tuples directly, preserving source-size
resolution as a separate future public gate. Complete predecessor Rust frame and
allocator baselines stay independent of the new depth-tagged tests.


The Lazy-family benchmark successor will retain the four pinned Greedy workload
byte sequences and eight existing profiles, then add the six explicit chain/row
profiles at each of depths one and two. Eighty total cases own deterministic
source/output vectors only in the std-only harness. The clock-free phase checks
complete frame bytes, resolved tuples, decoder reconstruction, exact allocation
counters and all prior Fast/DoubleFast/Greedy baselines before timing. Global
compression plus output destruction alone enter clocks; allocator surveys,
correctness checks and source generation remain outside. Each leaf backend and
all features receive identical source/request pairs. These harness records add
no production codec owner or unsafe implementation.


## Module 16: bounded no-dictionary reference BinaryTreeLazy2

This is a design for a successor to Module 15, not a supported encoder claim.
Public strategy code 6 stays unsupported until independent direct/ordinary C
observations, private frame integration and a successor admission fixture pass.
The pinned upstream revision remains Zstandard v1.5.7
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`. Relevant implementation is in
`lib/compress/zstd_lazy.c`: `ZSTD_updateDUBT`, `ZSTD_insertDUBT1`,
`ZSTD_DUBT_findBestMatch`, `ZSTD_BtFindBestMatch` and
`ZSTD_compressBlock_btlazy2`. The last entry uses the existing depth-two lazy
parser with binary-tree search; its sampling state is not a hash chain or row.

### Owners and index representation

A proposed `ReferenceTree<H, N>` owns independent initialized hash heads and
cyclic two-edge nodes using exact allocator values, with no allocator clone.
Hash heads use the existing checked Greedy key layout. A chain log C describes
2^C edge cells, hence 2^(C-1) nodes; it must not be interpreted as 2^C nodes.
The ring slot is `(logical_position + 2) mod 2^(C-1)` to preserve the upstream
window-start bias without pointer arithmetic or overflow near usize limits.
A node's two cells are ordinary indices, not pointers or references into storage.
All layouts and combined byte size are checked before the first allocation;
partial construction releases the first owner through its original allocator.

Upstream cells distinguish zero (empty), one (unsorted marker), and biased source
indices at least two. The Rust representation must preserve all three meanings.
One possible packed representation reserves `usize::MAX` for empty and
`usize::MAX - 1` for unsorted, excluding both from the bounded source domain.
The unsorted marker is legal only in the node's second cell during insertion;
when reversing an unsorted chain that cell temporarily becomes a position or
empty. A raw cell must be decoded by its current operation before being followed.
The source domain is independently checked before translating any C observation.
A copied `TreeEdge` identifies `(node_slot, side)` or a discard destination for
upstream's local `dummy32`. It never holds a mutable reference across another
table access. Sequential short borrows preserve aliasing rules even when ring
slots coincide. Plain edge/link/depth records need no artificial unsafe variant.

A dedicated opt-in `unsafe-reference-tree` leaf must share exactly the safe
contract and runtime validation, using checked initialized lengths before any
unchecked access. It must not reuse `unsafe-reference-chain` to silently change
a second structure's backend. Adding the leaf requires updating feature
inventories, every-leaf checks and benchmark configuration counts deliberately.
No unsafe parser or aggregate workspace implementation is proposed.

### State and search transitions

The state owns a copied checked execution plan, the tree, literal scratch and
sequence scratch. Its borrowed source stays alive throughout a one-shot frame;
all persistent links are source positions. Window floor and tree floor are
separate: the former limits match distance, the latter limits whether a ring
node still represents the candidate. A candidate can remain eligible for byte
comparison at or below the tree floor while traversal of its overwritten children must
stop. The observation/replay contract must preserve this distinction.

`next_to_update` can be ahead of the next parser query. The wrapper returns no
new tree match inside a skipped interval without backfilling it. Otherwise,
positions before the query are inserted as unsorted chains, with both cells
initialized. Search reverses a bounded suffix of that chain in the second cell,
nullifies the last still-unsorted candidate when necessary, and sorts stacked
candidates in reverse order with the evolving comparison budget. These writes
are retained even when the parser chooses a repeat or later falls back to a raw
or RLE block. No recursive host stack or separate heap stack is required.

Sorting/search maintain independent common-prefix lengths for the smaller and
larger branches. They begin byte comparison at their minimum and must prove the
prefix is within both relevant source extents. Equal-to-end candidates stop
before attempting a discriminator-byte read. Child following is bounded by the
comparison budget and checks candidate/source/window/tree invariants before
access, rather than trusting a previously valid fixture. Corrupt or cyclic state
must fail or terminate within an explicit bounded operation count; release-mode
assertions do not establish safety.

A longer byte match is not automatically selected: tree search compares a
four-times length gain against the offset high-bit cost. The arithmetic and the
initial offset-base value must match the pinned call path, with checked signed
operations. The farthest compared match end can advance the sampling frontier
even when its offset-cost proposal loses. Returning only the winning candidate
would lose this state. The direct protocol therefore records complete before/
after heads and nodes, frontier, selected length/offset, and relevant branch
witnesses independently of the lazy parser's final sequence list.

The depth-two parser can reuse checked `LazyCandidate`, `LazyDepth` and score
selection only after search is abstracted without changing existing chain/row
behavior. Main-search limits remain strict while eligible lookahead permits the
inclusive last eight-byte hash position. Both the skipped-frontier behavior and
lookahead-at-tail need direct C witnesses. Parser scratch preflight remains
transactional; a mid-search error poisons retained state. Committed entropy and
wire repeats remain owned by the frame workspace, with publication only after
output reservation and append succeed.

### Scope and gates

Start with a bounded no-dictionary contiguous-prefix source domain. Proposed
resource limits must be derived from node layouts and frozen before public
admission; do not infer new default-level support from strategy code alone.
Known-source adjustment can allow one extra chain bit for tree strategies and
must be independently checked against upstream. Hash selection, minimum-match
values and target-length behavior need observed coverage before being claimed.
Dictionary modes, optimal parsing, long-distance matching, multithreading,
large-index reduction and external sequence producers remain outside this
module unless separately designed and validated.

Freeze all 80 Module 15 complete frames and qualified allocator records as
permanent predecessor controls before changing production owners. First use a
small direct protocol with deterministic caller-owned guarded C arenas, bounded
operations, no arbitrary initial table contents and complete observations.
Malformed observations are tested only in offline consumers; do not feed
untrusted forged state into assertion-dependent C internals. Normal and full
ASan/UBSan captures must share source/recipe identities and have an independently
reviewed command manifest. Existing oracle scripts/fixtures stay immutable.
Ordinary frame trajectories follow the direct search proof and precede public
admission. The final matrix must cover the new leaf independently, all leaves,
std-disabled builds, selected foreign Miri checks and no-atomics caller codegen.
All-backend clock-free complete output/owner equality precedes native timing on
unchanged deterministic workloads and a separate safe/all repeat.

### Checked tree scalar implementation decisions

`TreeLayout` keeps the existing `GreedyKeyLayout` and validates edge-log 6..19
before calculating both independent usize allocation layouts and their sum.
This is a private owner-layout bound, not new public admission. It represents
2^C edge cells and 2^(C-1) nodes; permitting C19 retains at most 2^18 nodes.
`TreeLink` is a copied enum (empty, unsorted, logical position). Storage encodes
empty/unsorted as usize::MAX / usize::MAX-1; position construction excludes both.
Only a larger-edge cell may contain unsorted. `TreeEdge` is a checked node slot
and side or a discard destination; it never carries a mutable reference.

`TreeProbe` checks the inclusive eight-byte footprint and biased arithmetic,
then keeps raw window/node floors to preserve strict upstream comparisons.
Window candidates are strictly above max(prefix_floor+2, current+2-window).
Node traversal is strictly above current+2-(node_count-1), independently of
byte-comparison eligibility. Empty is a terminated traversal, unsorted is an
error in a sorted traversal, and future/self positions reject. These allocation-
free records share safe representation across every backend. Storage owners and
search consumers follow in separate work items; no public strategy is enabled.

The tree owner stores two independent initialized `Vec<usize, A>` allocations,
one for heads and one for all edge cells. Exact allocator values are moved,
never cloned. Unsorted insertion validates all three destinations and the old
head before the first write. Individual checked cell helpers select safe or
`unsafe-reference-tree` access after the same live-length guard. Reset reuses
both owners. The new leaf is added to default unsafe exclusion, the feature
inventory and every-leaf/no_std driver; historical benchmark/fixture metadata
remains immutable. A separate primitive owner measurement compares complete
normalized maps under safe/tree/all using identical prepared key sequences;
it does not claim end-to-end tree compression throughput.

### Deferred sorting implementation boundaries

The deferred sorter borrows source and independently owned table allocations.
Pending positions are inserted in order; a bounded descending unsorted chain is
reversed in larger-edge cells and then sorted oldest-first. The reversal stores
only copied positions, never aliased references. Its comparison budget grows
back to the original limit as stacked nodes are processed. Every stack successor
must increase while remaining below the main query; every original unsorted
predecessor must decrease. Malformed links reject with bounded work.

A sorting node uses its own strict byte-window floor but the main query's
maximum window/node floor for child traversal. This distinction preserves C's
terminal comparison of an old candidate without following overwritten slots.
Source comparisons check both suffixes; an equal tail stops before reading a
branch byte. Cursor destinations are copied TreeEdge values, including discard.
Sorting alone has no separately captured C phase snapshot: synthetic controls
qualify that work item, followed by exact full-operation maps when best-match
search is implemented. Existing captures remain unchanged.

`TreeSource` is an allocation-free borrowed range with independent prefix and
window bounds. Direct oracle source extents may exceed a window; ordinary frame
blocks retain their existing limits through checked conversion from GreedyBlock.
Each tree hash still requires eight logical bytes. This separation keeps direct
window-expiry fixtures representative without weakening ordinary block admission.
Best-match traversal owns only copied branch cursors and scalar common-prefix
bounds. It tracks selected length/position separately from farthest compared
match end, and preserves offset seed 999999999 until a strict cost win.

### Private parser and retained owner integration

A Tree variant joins the existing copied table plan, borrowed parser dispatch
and owned retained-table enum. It reuses LazyDepth::Two while always disabling
row mode. Private admission is separate (`from_tree_parameters`); existing
public strategy dispatch and family admission remain unchanged until ordinary
C frames and public tuples are qualified. Initial limits are W21/H19/C19/S8,
with C edge cells retaining half as many nodes. Checked TreeLayout supplies
actual independent owner layouts before any allocation or allocator cloning.

The parser sees only matches of length at least four; the primitive still returns
short hash-collision matches because their table/frontier effects are observable.
Tree queries ignore the chain skipping flag while preserving parser progress
state. Retained state keeps the existing block-start window clamp and 384/192
frontier adjustment from ZSTD_buildSeqStore. Empty/tiny blocks keep the existing
scratch and publication behavior. Shared transactional entropy/output ownership
and poison-on-search-error rules apply without an extra owner or unsafe layer.

### Ordinary tree trajectory projection

The tree fixture uses a distinct magic and fixed case/block counts around the
existing bounded frame-record encoding. It stores a fixed strategy selector 6
and zero reserved bytes; row metadata remains zero. Names, source, literals and
complete frames borrow immutable bytes. Sequence/delta vectors use fallible
reservations after bounded counts and byte extents are checked. The shared test
record gains explicit tree interpretation, never infers strategy from window
size or a name. Historical readers continue selecting their original chain/row
interpretation. Tree node markers are admitted only in odd edge slots during
independent full-map replay. Structural replay checks window/frontier and repeat
publication without calling the production parser; backend comparisons then
match actual sequences, literals and all map cells. Every complete C frame is
decoded independently; emitted Rust frames validate publication and source
reconstruction. Cross-backend Rust bytes must agree, but independent entropy
implementations do not promise C-identical compressed bytes.

### Public tree request dispatch

After the ordinary retained-state gate, public dispatch may select the existing
Tree plan for `ReferenceStrategy::BinaryTreeLazy2`. Known-size parameter
resolution occurs once, before W21/H19/C19/S8 admission and before cloning or
allocation. No public request adds a row owner, a source copy or a new allocator
boundary. Unsupported strategies 7..9 remain separate from implemented strategy
6 requests exceeding resource bounds. Original preset/selector evidence remains
immutable; versioned v5 mappings identify newly admitted and newly resource-
classified requests. Independent pinned-preset tuple reconstruction precedes
public C execution, whose generic adapter does not report applied tuples.

## Module 17 preparation: bounded no-dictionary reference Optimal

This is a design and observation plan for strategy 7, not a support claim. The
pinned counterparts are `ZSTD_match_t`, `ZSTD_optimal_t`, `optState_t`,
`ZSTD_rescaleFreqs`, `ZSTD_getMatchPrice`, `ZSTD_insertBt1`,
`ZSTD_insertBtAndGetAllMatches`, `ZSTD_btGetAllMatches_internal` and
`ZSTD_compressBlock_opt_generic` in facebook/zstd v1.5.7. Start with `optLevel=0`
(`ZSTD_compressBlock_btopt`); Ultra/Ultra2, dictionary-derived prices, LDM,
workers and numeric streaming remain separate later work. Proposed private
resource bounds W21/H19/C19/S8 require their own independent public admission
study before enabling anything.

### Owners, records and boundaries

| Proposed Rust role | Ownership and upstream counterpart | Required invariants |
| --- | --- | --- |
| Optimal search plan | Copied validated geometry; no allocations | Independent known-size admission; optional three-byte table only for minimum-match 3; no implicit row selection |
| Sorted search heads/edges | Reuse independently owned tree storage where its cell contract fits, not the Lazy2 traversal rules | Paired biased slots, checked live length and descending historical links; no unsorted marker generated by optimal insertion; every retained floor compared by the actual optimal rule |
| Three-byte heads | Optional separately owned initialized positions (`hashTable3`) | Log is min(window log, 17) for the pinned CCtx path; own update frontier; physical four-byte C loads may only be emulated by checked logical reads; do not import the lazy-family minimum-four rule |
| Candidate match buffer | Initialized fallible owner of copied match records (`ZSTD_match_t`) | Encoded offset intent remains distinct from distance; each accepted record strictly increases match length; checked count within the captured 4,099-entry capacity; no pointer into mutable owners |
| Frequency model | Owned literal[256], LL[36], ML[53], OF[32] counts with copied sums, base prices and a pricing mode (`optState_t`) | Nonnegative bounded counts; sums agree with active tables; checked downscale/add arithmetic; symbol indices validated before access; no borrowed C entropy table |
| Price/path workspace | Initialized bounded owner of copied path entries (`ZSTD_optimal_t`) | 4,096-position search horizon plus three checked scratch slots; explicit reachable/unreachable state; finite checked signed cost arithmetic; repeat snapshots include all three offsets |
| Path reconstruction | Copied indices/ranges and fallible sequence scratch | Each predecessor decreases by a positive checked extent; terminal literal-only stretch is explicit; copied records before any overlapping rewrite; no aliased references to the price workspace |
| Retained optimal state | Own search tables, optional short-key heads, frequencies and reusable candidate/path scratch; borrow one source slice | Exact moved allocator identities, reset/reuse contracts and fallible construction; poisoned context after partial search/output failure; wire repeats published only after complete compressed block output |

Copied geometry, match records and path records share their safe value
representation. Do not introduce unsafe operations merely because a record is
performance-sensitive. Tree cell access keeps its existing isolated opt-in leaf.
Before implementation, give any new performance-sensitive short-key, candidate,
frequency and path owner a separately reviewed safe/access backend contract and
independent opt-in feature (reuse an existing owner only where the full contract
matches). Runtime live-length/domain checks must precede every unchecked access.

### Algorithm distinctions that require new observations

Optimal template minimum clamps to 3..6, but the primary tree hash uses widths
4..6 (ZSTD_hashPtr defaults to hash4 for template minimum 3). Only the separate
short-key table uses hash3. Minimum-match 3 is real three-byte matching
and has a separate hash table and update frontier. The generic sequence format
can represent length-three matches, but the current lazy-family parser's
minimum-four filter must not be reused. A selected encoded repeat intent depends
on literal-length-zero and all three repeat offsets, including rep0-minus-one;
zero distance is an absent candidate, never a readable source location.

The existing BinaryTreeLazy2 `TreeProbe` floor policy is not an optimal-search
specification. Optimal candidate checks can include a prefix-boundary position
that a strict lazy-tree floor excludes. Sorted insertion also differs from lazy
deferred unsorted reversal. Reuse physical storage only after direct observations
qualify boundary eligibility, overwrite floors, skip frontiers and complete maps.

The no-dictionary frequency model starts from first-block literal counts and
pinned base LL/ML/OF distributions. The implementation uses predefined pricing
when source size is **at most eight**, despite the nearby upstream comment's
strict-less-than wording. Later blocks rescale accumulated statistics; parsing
can change search/frequency state even when entropy output falls back to raw/RLE.
Capture this distinction independently from committed wire repeats and entropy
state. Do not infer parser-statistic rollback from output-repeat rollback.

Statistics updates do not refresh cached base prices themselves. The parser
refreshes them after emitting a selected sequence group, while rescaling also
refreshes them. Observation requests must distinguish update from refresh and
retain stale-but-intentional base prices between those phases.

Strategy 7 uses whole-bit weights scaled by 256. The cost model is separate from
actual Rust entropy selection. Checked wider intermediate arithmetic may express
C's finite signed costs, but must preserve ties and optLevel=0 early-abort rules.
`ZSTD_MAX_PRICE` is an unreachable sentinel, not permission to overflow arithmetic.
The path workspace stores a match followed by literals during forward search,
then emits literals followed by a match during backtracking; those two meanings
must have distinct checked APIs or typed phases.

Target length is operational here: sufficient length is capped at 4,095 and can
trigger immediate selection, unlike the target ignored by Lazy/TreeLazy2. Public
zero still means a preset default, while an internal helper may observe an
explicit applied zero; freeze those separate contracts. Cover source and match
lengths around 8, 4095/4096, 65535/65536 and block limits before extrapolation.

### Evidence and rollout order

Preserve all 24 completed tree benchmark frames and qualified exact-owner rows
as permanent predecessor tests. Freeze source-only frequency/price recipes and
transport before C execution, then independently replay complete tables/sums/
prices with forged-record controls. Implement checked scalar pricing and owners
only after those expectations are fixed. Next capture sorted-tree/all-match
operations, including every match record and both update frontiers, before
implementing search; then qualify path transitions and backtracking separately.

Ordinary blocks must expose full frequency, candidate, path, search-map and
repeat publication state. Public strategy 7 remains unsupported until private
ordinary conformance, allocation-failure boundaries and independently resolved
public tuples pass. Every enabled unsafe leaf, true no_std, selected foreign/
no-atomics checks, cross-decoding and identical-workload measurements remain
required. No price estimate, C/Rust entropy-byte parity or complete upstream
migration is implied by an intermediate checkpoint.

### Frozen pre-Optimal tree regressions

The immutable tree clock-free archive now projects to twenty-four complete frame
and allocator rows and thirteen distinct frame extents. The bounded offline
reader checks every one of the seventeen archived backends and reconstructs
expectations without executing a codec. Normal and optimized controls each reject
nineteen changed frame, row, provenance or inventory inputs with no children.
An initial control wrote the already-zero first frame offset; the corrected
control guarantees a different value. Original failures are retained.

Seven Rust tests pass in safe/tree/all/release-all configurations (28 tests),
replaying all complete frames, resolved tuples and target-qualified owner rows.
Safe/all focused Clippy and formatting pass. These additive tests postdate the
immutable Module 16 full-matrix snapshot and are qualified separately in
`evidence/2026-10-13-tree-baseline-v2.json` and its source/log archive. They do not
change codec code or imply Optimal support.

### Optimal scalar price checkpoint

Before frequency ownership integration, an allocation-free scalar module may be
compiled through a focused harness and a standalone no_std wrapper. `SymbolPrice`
is a copied pair of checked whole-bit base/frequency weights; it owns no table,
allocation, index link or borrow and therefore shares its safe value representation
without artificial unsafe operations. Symbol lookup uses the existing attributed
LL/ML format intervals and rejects out-of-domain lengths before indexing.

The raw-literal helper borrows a typed 256-cell count array only for the call,
checks every consumed scalar statistic, and retains nothing. No cache is refreshed
implicitly: the later retained frequency owner must supply the correct phase's
base prices. Checked sums reject the unreachable 2^30 sentinel and overflow.
Predefined prices, disabled literal costs, full-block LL adjustment and dynamic
match penalties remain distinct APIs. Initial code is not added to public dispatch
or the library module graph while the previous milestone's immutable matrix runs;
its source, no_std compilation and all captured query comparisons are independently
qualified. Frequency ownership and its own optional unsafe feature follow later.

### Optimal frequency owner and isolated integration branch

`OptimalFrequencies<A>` owns one fallibly initialized `Vec<u32, A>` containing
exactly 377 live cells, partitioned into fixed literal/LL/ML/OF ranges. A single
moved allocator owns the contiguous allocation; no allocator clone, source copy,
interior raw pointer or reference is retained. The four sums and cached prices,
pricing mode and initialized/dirty flags are copied scalar state. Reset preserves
the same allocation and clears every count and phase. Disabled literal mode is
fixed per owner and retains zero literal statistics.

The independently opt-in `unsafe-optimal-frequencies` feature replaces only
checked scalar cell reads/writes after runtime live-length/index checks. The
safe implementation has the same API and errors. Immutable views used by price
queries remain scoped borrows; no ownership or lifetime escapes. Snapshot values
copy complete tables for focused conformance tests and need no artificial unsafe
representation.

Each update first validates lengths/encoded offsets and preflights histogram,
frequency and sum arithmetic before mutation. Every counter remains below
u32::MAX so weight(stat+1) is valid. Invalid requests, overflow, malformed storage
or failed initial allocation leave published model state unchanged. Successful
updates mark cached prices dirty; queries reject that phase. Explicit refresh or
begin-block rescale updates cached prices. First-block initialization and later
base-one rescaling follow the independent captured lifecycle; raw/RLE wire repeat
publication remains a separate future parser concern.

This isolated branch integrates only private scalar/frequency modules and their
Cargo feature. Public strategy 7 stays unsupported. Full tree-milestone tests and
measurement use their prior immutable source; this branch requires focused
safe/isolated/all/release conformance, allocator failures, every-leaf no_std,
Clippy and complete source/feature evidence before its own commit and later merge.

### Tiny sorted-match observation replay

The independent Python replay owns zero-initialized integer heads, paired nodes
and optional short-key heads. Both frontiers begin at native index two. It counts
equal suffix bytes directly from immutable source, without copying C's common-
prefix optimization, pointer aliases or loads. Pending smaller/larger links are
optional (slot, side) values; absence discards an expired link. Catch-up and query
insertion publish frontiers differently. Match intents remain encoded offBase
values, and repeat bytes use independently resolved distances.

Complete copied before/after maps and frontiers are bounded observations. Replay
recomputes every record from requests; canonical comparison rejects type changes
and unknown fields. Hand-derived repeat, short-key, full-tail and skipped-frontier
cases precede C capture. This test-only owner does not enable public Optimal.

### Broader direct short-key observations

The ZOPMB02 source owner caps at524288bytes and16queries, with window10..21.
Primary/paired storage remains at most4096cells; the optional short-key owner
uses exactly min(window,17), up to131072cells. All maps and both native-biased
frontiers are retained. Source/packet/owner copies and guards fit a reviewed
static4MiB observer budget; C execution requires its own bounds/preflight gate.
Independent replay compares equal4096byte source chunks, then the first unequal
chunk bytewise, and must agree with scalar tiny replay before broader capture.
No production owner or unsafe leaf is introduced by this observation protocol.

### Canonical sparse wider match fixtures

Broader C observations still retain complete maps. Their binary projection uses
an explicit cell count plus every nonzero (index, value) in strictly increasing
index order. Omitted cells mean zero, so there is exactly one encoding for a map;
duplicate indices, explicit zeros and out-of-bounds entries are rejected. The
independent Rust reader owns only the sparse entries with fallible allocation,
borrows source/name bytes, and validates hash buckets, complete state continuity,
frontier bounds and every candidate's source bytes. No codec algorithm or unsafe
backend is involved in this test-only representation. Dense owner comparisons
can iterate the unambiguous complete map without retaining many repeated zeros.

### Private sorted Optimal search contracts

`OptimalSearchLayout` is a checked scalar geometry: window 10..21, primary and
paired-edge logs 6..19, search 1..8, minimum 3..7, target at most 131072. Primary
keys reuse `GreedyKeyLayout` (minimum 3 hashes four bytes). Optional short keys
hash exactly three bytes with logs 6..17; ordinary contexts select min(window,17),
while direct observation tests may select smaller logs. No table exists when
minimum exceeds three. Shared scalar records need no unsafe representation.

`OptimalSource` borrows an immutable contiguous source for the search lifetime,
with at most u32::MAX - 2 bytes, preserving the observed native index bias of two.
The direct primitive leaves eight bytes at a query and checks repeat distances,
length-to-beat and source ranges before mutation. Zero repeats are admissible
direct requests but ineligible matches; rep0 minus one uses explicit wrapping
subtraction and is rejected by distance eligibility when rep0 is zero.
Prefix/window eligibility is
separate from paired-node retention. Ordinary startup/block state remains a later
integration gate. Matching reads only initialized logical bytes without padding.

`OptimalShortKey` owns one initialized `Vec<usize, A>` with the exact moved
allocator, independent of primary heads/edges. Empty is usize::MAX; every live
value is a source position. Disabled storage is a zero-length vector retaining
the allocator and performs no allocation. Reset fills enabled cells with empty
and allocates nothing. `unsafe-optimal-short-key` changes only checked cell access;
all bounds are enforced at runtime in both implementations. Hashing is shared
safe scalar arithmetic, corresponding to ZSTD_hash3 and hashTable3.

`OptimalMatches` owns 4099 initialized candidate records in `Vec<_, A>`, plus a
live prefix length. `OptimalMatch` is a copied offBase/length pair (nonzero intent,
length at least three); direct observations can exceed a block's 128 KiB length.
Appending requires strictly increasing lengths and available initialized space;
failure leaves the live prefix unchanged. Clearing retains storage. Its separate
`unsafe-optimal-matches` feature selects only runtime-guarded access. Empty
initialized placeholders never escape the live prefix. This corresponds to
the ZSTD_match_t matchTable workspace, with the reviewed 4099-cell observer bound.

The sorted search composite reuses `ReferenceTree`'s two independent owners and
its existing unsafe leaf. Sorted traversal rejects the deferred-tree unsorted
marker. Main and short-key frontiers are logical source indices (initial zero);
no pointer or source alias is stored. Repeat publication remains caller-owned.
Malformed requests are rejected before mutation; errors discovered during table
traversal poison the composite until reset. Reset clears all owners and frontiers
without allocation. It will implement ZSTD_insertBt1 / ZSTD_insertBtAndGetAllMatches
and ZSTD_BtGetAllMatches for the observed contiguous, no-dictionary domain.

### Scripted path observation ownership

The first path observer owns a zero-initialized 4099-cell C path workspace and
complete frequency/sequence workspaces; it borrows an immutable guarded source
of at most 512 bytes. A bounded script owns at most 64 positions with eight
strictly increasing source-valid matches each. Its callback replaces only match
selection in the pinned generic parser, preserving original path transitions.
Every observation retains initialized cells, including stale fields, plus explicit
phase and coordinate tags. Independent replay owns integer cell records and a
separate forward/reconstruction phase. A cell's predecessor must be read before
any overwrite during reconstruction. The bounded protocol cannot qualify the
4096-position horizon or ordinary CCtx startup; both remain explicit later gates.
See `reference-optimal-path-observer.md` for exact observation and rejection rules.

### Wider path snapshot deltas

The wider path observer retains a full initialized path owner of at most 4099
cells and a separate initialized previous-snapshot owner. An event enumerates
exactly the changed cell indices in increasing order with complete seven-word
records. Missing indices preserve the previous value; the initial snapshot is
all zero for each padding pass. Explicit zero-valued changes are valid, while
unchanged entries and duplicate indices are noncanonical. Replaying the deltas
recovers the complete owner, including stale fields, at every event. The emitted
view never mutates the parser's owner. This bounded representation avoids retaining
quadratically repeated full maps while qualifying the 4096-position horizon.

### Canonical path fixture projection

The test-only path fixture borrows names and source bytes and owns bounded event
records with fallibly allocated deltas. It combines the two immutable audited C
captures without regenerating observations. Each event retains all scalar tags,
coordinates, optional stretch/reverse/query data and ordered candidates. Path
changes carry an index and all seven cell words. Frequency changes address a
386-word view: 377 counts, four sums, four cached prices and one predefined flag.
Only changed entries occur, strictly ordered, against zero-initialized per-case
baselines. Sequence output is append-only with an explicit live count. Replaying
these records recovers every initialized path/frequency cell and emitted sequence
at every event. No production storage or unsafe feature is introduced.

The independent Rust reader bounds the complete byte extent before allocation,
checks canonical changes, stage/phase/optional-field shapes, source/candidate
ranges, sequence source equality and repeat intent, frequency sums, refreshed
prices and reconstruction indices. The binary identifies small versus wider
protocol bounds explicitly. Unknown flags, stale baselines, duplicate/unchanged
entries, impossible live counts and trailing bytes are rejected. Python projection
first validates both padding passes and builds against independent replay, then
stores one equivalent copy. All original raw maps remain in their archives.

### Checked Optimal path values and ownership

The next private layer corresponds to `ZSTD_optimal_t` and the in-place shortest
path pass in `ZSTD_compressBlock_opt_generic` (pinned v1.5.7). `PathCell` is a
copied seven-word record, not an allocation or pointer. It preserves initialized
but stale fields exactly: setting an unreachable price does not erase offset,
length or repeats. `PathPrice` distinguishes the unreachable sentinel (2^30)
from finite costs below it; checked signed adjustments reject overflow, negative
results and sentinel collision. This is shared safe value code for every backend.

A forward view consumes a finite cell as a match followed by literals, with a
checked predecessor that strictly decreases for a nonempty match. A reverse view
consumes a cell as literals followed by a match and checks its source advance.
Raw unused/stale fields are bounded but are not interpreted as valid repeats or
match intent until the relevant view consumes them. Fresh zero-initialized cells
must remain representable. Repeats are copied words; their source-validity and
publication are checked by the parser/search boundary, not by inventing a
nonzero invariant for unused cells. The block-local contiguous source bound is
131,072; this layer admits no dictionary/external source history.

`PathIndex` validates the 4,099 initialized slots independently of match length.
The 4,096 forward horizon and three scratch slots are not a maximum match length:
an immediate long match can advance to the block end while reconstruction uses
only `cur + 2`. `ReverseCursor` tracks copied source/destination indices. Reading
a predecessor occurs before an overlapping destination write; every nonterminal
step checks a positive match-plus-literal extent, decreasing predecessor and a
remaining destination strictly above that predecessor. A terminal literal-only
entry does not subtract its literal count from the predecessor coordinate.

`OptimalPaths<A>` will own one exact moved allocator and an initialized vector
of 4,099 cells, using fallible allocation. A whole-owner phase (`Forward` or
`Reconstruction`) prevents accidental use of the opposite typed view; transitions
are explicit and reset zeroes all cells without allocation. Raw snapshots are
copies, never aliases across mutation. Safe and `unsafe-optimal-paths` access
will have the same contract, with runtime slot/length checks preceding every
unchecked operation. Failed indices, values and phase transitions leave the
owner unchanged. Finite record implementation and owner implementation remain
separate checked work items; neither enables public Optimal dispatch.

### Private scripted Optimal path decision engine

The next engine borrows one immutable block-local source and owns independently
moved frequency, candidate and path allocators. A provider writes checked sorted
candidates into the existing bounded owner for each copied query; it does not
borrow the mutable path/frequency owners. Source equality and resolved repeat
history are validated before consuming provider output. This provider boundary
is deliberately separate from ordinary sorted-search startup qualification.

The engine emits copied transition metadata to a fallible observer, with shared
views of initialized path/frequency storage and the current query candidates.
Only sequence events carry one newly emitted record; the observer owns any output
allocation. Algorithm decisions use production prices and records, never fixture
expected values. Captured events are compared after each transition, including
all cells, complete statistics, future path repeats and committed wire repeats.
The frequency cache stays unchanged during reconstruction updates and refreshes
only after the selected sequence set, matching the captured lifecycle.

A run starts with fresh [1,4,8] repeats and one source; one successful run marks
it finished. Provider, observer or internal errors after entering a run poison the
engine, because path/statistics and externally emitted output may be partial.
Reset clears owned working state without allocation and returns to fresh state;
it cannot undo external observer/provider side effects. Invalid construction and
repeat runs fail before callbacks. Terminal results contain trailing literals,
sequence count and committed repeats. The only admitted mode is scripted fresh
optLevel0, minimum parameter 3..7 (effective 3 or 4), target <=4095, source 9..131072,
no dictionary or external history. Public Optimal remains unsupported until
ordinary retained-state search and full-frame qualification complete.

### Fresh ordinary Optimal startup observation boundary

The next additive oracle must preserve the original match-provider selection and
run the pinned CCtx advanced initializer with explicit btopt parameters. It will
borrow one 64..512-byte source, retain actual applied parameters, all search maps
and frontiers, complete path/frequency state, every candidate/sequence/repeat,
and the complete ordinary frame. Requested and applied parameters must agree;
a source-size-adjusted or fallback strategy is not silently treated as btopt.
The initial scope is one fresh block; retained multi-block behavior is separate.

The independent source-only replay composes the reconciled integer sorted-search
model with the already qualified path model. Queries are generated by the path
algorithm, not supplied as scripts. Fresh [1,4,8] repeats are recorded even when
they exceed available prefix history; only eligible distances may read source.
All indices retain the upstream bias of two. The complete search state is carried
at every event, with the pre-query state additionally retained at query events.
No new production query admission follows from this predictive model alone.

The old direct-match guard repeats <= position + 2 remains unchanged. It prevents
an unused noDict `dictBase + repIndex` expression when unsigned subtraction wraps.
The ordinary observer must not remove or replace upstream repeat eligibility,
pointer-overflow attributes or provider selection to bypass that issue. Review
must document the unchanged CCtx window setup and original attribute, and prove
that observer-owned reads use checked source ranges. No claimed sanitizer result
can prove that every unused C pointer expression has ISO C object provenance.

For deterministic complete first-use cells, the new oracle will use a bounded
zero-initializing custom allocator through the existing CCtx custom-memory API.
This initializes bytes supplied to upstream; it does not rewrite codec decisions.
Record exact allocation extents/failures and ensure all owners release. Later
blocks must retain actual state rather than re-zeroing it. Both source-padding
passes and normal/instrumented builds must agree, with independent frame decode
and forged-record controls before Rust search-startup admission changes.


### Fresh startup executable capture records

The additive capture plan owns immutable argv records, timeout/output limits and
an exact per-request artifact map. A request creates two complete frame files
and two allocation ledgers in its build-specific artifact directory. Invalid
packets must fail before metadata and create no artifacts. The runner owns each
child and bounded pipe buffers, closes both pipes, kills overdue children and
preserves failed command diagnostics. Dependencies and effective predefines are
validated immediately, before any observed helper execution, not just post hoc.
The reference closure is the pinned C/header inventory; the one generated opt
unit replaces only its original translation unit, retaining 25 others.

The independent auditor borrows immutable packets and logs and regenerates every
complete startup event using the integer search/path composition. It separately
checks canonical frame header/content-size/block extents, raw/RLE source bytes,
all allocation ledger integer domains, release, and equality of both padding
passes and both builds. Compressed payload decoding is a separate Rust fixture
gate; structural inspection is never described as full decoding. The C helper
already decodes each complete frame with the pinned decoder. Capture does not
qualify retained block publication, where raw fallback may discard path choices.

All new capture files are additive. Existing predictive recipes, source model,
observer and predecessor source manifests remain unchanged. If executable
preflight exposes a helper problem, preserve that failure and use a versioned
successor helper with an explicit reviewed diff. No search admission changes
precede the successful state, frame, and forged-evidence gates.


### Fresh startup fixture projection

The proposed `ZOSFX01` envelope stores twelve cases with copied W/H/C/S geometry,
complete frame bytes, separate normal/sanitized allocation triples, one embedded
`ZOPFX01` path case and canonical complete-search deltas per event. Existing path
reader invariants can therefore validate path/frequency/repeat/source state without
editing frozen predecessor fixtures. Its callback list is the unique observed
nonempty query projection, not a callback supplied to C: repeated positions must
have identical candidate lists or projection rejects. The ordinary Rust integration
must ignore that list for candidate production and compare its own search output.

Each embedded path case borrows name/source, owns fallibly allocated event deltas,
and inherits the independent path reader's source-match and reconstruction checks.
Search arrays flatten heads, paired node cells, optional short heads and the two
biased frontiers. Canonical deltas include every changed word, with ordered unique
indices and no unchanged entries. Pre-query snapshots must equal the previous
complete search state; each event retains its complete post-state through deltas.
The independent Rust reader validates geometry, fixed extents, raw-index domains,
canonical ordering, event correspondence and exact EOF before exposing fixtures.
No new unsafe backend is needed for test-only records or borrowed fixture spans.


### Private fresh-query admission after capture qualification

The qualified fresh domain is a separate checked entry, not a weakening of
`OptimalSource::validate`: source 64..512, W10..12/H6..9/C6..9/S1..4, minimum
parameter 3..7, target <=4095, and the ordinary short table only for minimum 3
with log equal to window. Queries begin at one, retain the eight-byte footprint,
use the effective minimum as their beat, and carry three positive repeats bounded
by source length. Unavailable fresh distances remain values; checked subtraction
rejects them before any source access. Existing direct queries retain <=4096 and
<=position+2 repeat restrictions. Both entries share the already qualified integer
search implementation and its poison/reset semantics, with no new allocation.

Full fresh fixture tests will compare every query result and complete search map,
including unchanged maps between path events and both frontiers. Rejected fresh
geometry/query bounds must preserve storage and live candidates; internal failure
still poisons until reset. The new entry remains private and does not establish
retained blocks or public strategy 7 admission. Existing safe/unsafe leaf contracts
and deterministic predecessor fixtures remain applicable.


### Private fresh search/path composition

`OptimalParser::parse_fresh` will borrow an existing `OptimalSearch` for one
complete fresh source. The parser retains its independent frequency, path and
candidate owners; search retains heads, paired nodes, optional short heads and
its own candidate owner. Six allocations (seven with minimum 3) therefore remain
explicit and independently allocator-parameterized. A private provider adapter
copies the bounded search candidates into the parser's existing live prefix;
no parse/reset allocation is introduced and no source bytes are copied.

Before parser mutation, the entry validates captured fresh geometry, exact borrowed
source identity, effective minimum/target agreement, and untouched search frontiers,
candidates and poison state. Existing scripted parsing stays separate. Parser
errors poison parser reuse; search errors also poison search. Resetting only one
component cannot authorize reuse when the other still contains prior query state.
The caller explicitly resets both owners to retry. Callbacks may already have
published output before failure; neither parser nor search rolls back caller data.

Full C fixture comparisons cover path/frequency scalars at every event and complete
search maps before and after every actual query, while candidates are generated
by Rust search rather than read from the embedded observed list. Allocation tests
must include moved and borrowed non-clone owners, every creation failure boundary,
no allocation during parse/reset and exact deallocation identity. This private fresh
composition cannot commit retained frame repeat/frequency state or enable strategy 7.


### Retained ordinary Optimal observation design

The next additive retained protocol owns one immutable contiguous source of at
most 1,024 bytes and a schedule of two to four 64..512-byte blocks. Explicit
W10..12/H6..9/C6..9/S1..4 geometry ensures the complete source remains within one
window; this initial retained domain makes no window-expiry claim. No dictionary,
LDM, worker, producer, or post-sequence splitter is admitted. Two padding passes
create new CCtx owners, but each pass preserves its ordinary owners across all
Continue calls and final End. Existing fresh captures and query guards are frozen.

The observer retains all 515 potentially used path cells, complete frequency
counts/sums/cache, complete search maps/frontiers, every query/candidate/sequence,
prev/next compressed-state owner identities, and repeat arrays before/after each
call. Path cells and frequencies persist even when entropy compression selects a
raw or RLE block. Candidate repeats are copied from the current committed owner
at the start of each block. Only a compressed block swaps entropy/repeat owners;
raw and RLE output retain the previous committed repeats. Frame bytes, block kind,
block extents and Rust decode independently bind the publication interpretation.

`nextToUpdate3` is block-local and is initialized from the main search frontier at
each parser entry; the short-head allocation itself persists. The independent
model must represent that reset rather than retaining the prior local frontier.
The block-local last-stretch record starts at zero on every parser invocation,
while the owned path table does not. The first block starts at global position one;
later blocks start at their global block boundary, where zero literals and repeat
history can immediately participate. Search comparison is bounded by the current
block end, even though source backing bytes for future blocks already exist.

The predictor borrows current-prefix source for global integer search, current
block source for literal pricing, and carries shared search/path/frequency state
between block models. Publication hypotheses are frozen recipe fields, not runtime
instructions to C. The eventual auditor reads actual wire block kinds and requires
those hypotheses to agree before applying committed-repeat state transitions.
Both source padding and normal/sanitized C must agree; allocator records keep the
already derived sanitizer redzone adjustment. Invalid transport, forged state,
changed publication owner/repeats and altered frame output must reject offline.


### Retained fixture and bounded block admission design

The planned `ZORFX01` fixture borrows each source and complete frame, owns bounded
block/event records, and stores separate ordinary/sanitizer allocation triples.
Per-case geometry and exact block schedule precede chronological blocks. Each
block records start/length, wire kind, committed owner before/after, incoming and
candidate repeats, both owners after publication, pre-call main frontier, and
all path/frequency/search deltas. Every delta baseline persists across blocks;
only the per-block sequence list begins empty. This preserves untouched stale
path cells and retained frequency/search state without re-zeroing them.

The independent reader must bound source <=1,024, two to four 64..512-byte blocks,
515 path cells, ordinary short geometry and all allocation/count arithmetic before
allocation. Global literal/match positions must stay inside the current block end,
with prior matches allowed anywhere in the consumed prefix. Reverse cursor bounds,
finite prices, complete frequency sums and canonical delta ordering remain checked.
The initial event of each later block permits only main/local-short frontier changes
in search state, derived from the strict 384/192 rule; query pre-state is the previous
complete state. Compressed publication swaps owner and commits candidate repeats;
raw/RLE publication retains the prior committed owner while preserving candidates
in pending storage. Complete frame decoding is independently compared to source.

Private retained production admission follows successful fixture qualification.
It must preserve the current fresh/direct APIs and never expose unqualified history
or window-expiry domains. Search needs a checked current block end so hash/match
reads cannot use future backing bytes. Path parsing needs a bounded global block
range, retained tables/frequencies, incoming committed repeats and a zeroed local
last stretch. A coordinator owns the publication phase and all original allocators;
argument errors precede mutation, and internal/callback failures poison reuse until
an explicit full reset. Empty/tiny blocks, larger sources and public frame dispatch
remain separate gates rather than implicit generalization from this small domain.

### Private retained source bounds and publication coordinator

`OptimalSource` retains one immutable full backing slice plus a checked visible
prefix end. All key, byte and match operations use that prefix; source identity
checks still compare the full borrowed slice. This scalar value needs no unsafe
representation. `OptimalSearch` adds an optional active `(start,end)` range,
validates the retained 128..1024 source / 64..512 block domain before mutation,
and performs the observed strict main-frontier catch-up and local-short reset
at each contiguous entry. Direct and fresh query APIs reject retained mode;
reset restores full visibility and clears the mode with the original owners.

The path parser will keep full source identity but use explicit active block
start/end for every source/match/sequence bound. A retained entry preserves path
cells and frequencies, initializes pricing from only the current block, and
resets local sequence count, anchor, repeats and last stretch. Fresh/scripted
entry contracts remain unchanged. The parser validates candidate source bytes
against the current prefix even though future backing bytes are accessible to
the owner. A caller cannot enter another block after a poisoned parse.

A private `RetainedOptimal` coordinator will own the parser and search (the same
six or seven allocator owners), a copied fixed four-slot block schedule, two
repeat triples, previous-owner index and an explicit ready/pending/finished/
poisoned phase. Constructor checks the full two-to-four block schedule before
mutation. Parsing creates pending candidates; only a later publication decision
(raw/RLE/compressed) advances the schedule and optionally swaps the repeat owner.
This mirrors ordinary `ZSTD_buildSeqStore` and compressed block-state publication;
it does not implement entropy compression or expose public strategy selection.
Invalid phase/schedule arguments leave state unchanged; internal or observer
failure poisons reuse until resetting every owner. Parse, publication and reset
introduce no allocations. Existing per-owner unsafe features select storage only;
all coordination and source-bound logic remain safe Rust.

### Wider retained sparse observation design

Before widening production admission, an additive `ZOPRW02` request will retain
one contiguous source <=16,384 bytes, two to four blocks of 64..8,192 bytes,
W10..16/H6..12/C6..12/S1..6, minimum 3..7 and target <=4095. Total source must fit
the selected window, so this stage still excludes history expiry. Ordinary
short-head geometry is the selected window. All 4,099 path cells participate;
this covers the actual 4,096-position horizon with its reconstruction slots.

The observer owns fixed initialized previous-path and flattened previous-search
arrays outside the CCtx. It emits canonical increasing changed-cell deltas rather
than full arrays. These observer baselines reset only between padding passes;
they persist across blocks. A pre-query sparse record is measured against the
preceding complete search state and must be empty. Post-query deltas advance the
baseline. Frequency counts/sums/prices and per-block sequence records remain
complete. Every source access and owner extent retains the existing guards and
unchanged-provider instrumentation; no parser statement is substituted.

The independent model uses the already qualified integer transitions, separately
bounded wider request validation, and independently computed sparse snapshots.
It must reproduce the existing sixteen captures after lossless projection into
the sparse representation before any wider native execution. New source-only
recipes must demonstrate long matches, high path indices, sufficient-length and
horizon decisions, block-boundary queries and repeat rollback where predictable.
Wire publication remains an explicit hypothesis checked against actual frames.
Source, packet, event, per-command output and aggregate campaign limits must be
frozen before native execution. Fixture projection will preserve all changed
cells and independently validate unchanged prior state; no sampled-state claim
may replace complete-state qualification.

### Wider complete fresh-block observation gate

Public one-shot admission also needs a complete fresh source, independently of
manual retained Continue scheduling. A subsequent additive `ZOPFW03` transport
will contain exactly one 64..16,384-byte block and its full known-size source,
W10..16/H6..12/C6..12/S1..6, minimum 3..7, target <=4095 and source inside window.
It retains the same complete sparse path/search representation and guarded owner
observations. The one block is passed to ordinary Continue and terminated by an
empty final raw block; every complete frame is independently decoded. No native
provider is changed. Fresh initialization and sufficient-length behavior remain
explicitly distinct from retained owner publication.

The independent block model can borrow the request's entire immutable source,
because no future block exists. Its scalar transitions and 4,099 path cells are
unchanged, including long matches that bypass path seeding. It must reconcile
all existing fresh startup observations and freeze wider full-source hypotheses
before native execution. The source cap alone does not authorize public
parameter selection, allocation cloning or frame/token integration; those
remain separate plan, owner, mapping and interoperability gates.

### Wider retained production admission

The qualified ZOPRW02 source domain extends only retained admission: source
128..16,384, blocks 64..8,192, W10..16/H6..12/C6..12/S1..6, source <= window.
Fresh and direct-query contracts stay separate. `OptimalSearchLayout` checks
these complete scalar bounds before retained mode changes; the existing full
backing/visible-prefix representation and contiguous entry invariant remain.
All heads, paired edges and short heads retain their original independently
owned initialized storage and opt-in unsafe features. No new allocation or
unsafe operation is needed for the wider scalar admission.

The parser still owns exactly 4,099 path cells and validates every transition
against the active block end. Long matches may bypass path seeding; horizon
reconstruction must match native cell 4096 without confusing match length with
path capacity. The coordinator keeps the same fixed four-slot schedule, two
repeat owners and explicit pending phase. Wider source/block limits do not
authorize an extra block, future-byte reads, window expiry or fresh public use.
Separate search-map, path/publication, and ownership/negative gates precede
complete feature/platform qualification.

### Bounded Optimal frame plan and token ownership

After complete fresh observations and private parsing are qualified, an
`OptimalExecutionPlan` will hold copied resolved parameters, source size,
checked search layout, frame settings/bound and explicit owner byte counts.
The initial scope is one complete known-size source of 64..16,384 bytes,
inside its window, W10..16/H6..12/C6..12/S1..6, minimum 3..7, target <=4095.
Parameter resolution happens once before dispatch; invalid source/tuple,
unsupported strategy, operational resource bounds and checked allocation
layouts are validated before allocator clones or requests. The current
small-input and larger-source exclusions are explicit resource policy, not
claims about upstream format validity. Retained scheduling remains private.

An `OptimalMatchState` will own the qualified fresh parser/search owners and
two caller-owned `Vec` scratch buffers for literals and `SequenceWithOffset`.
It borrows the complete immutable source. Seven parser/search allocator
identities remain separate (the optional short owner may allocate nothing);
literal and sequence vectors retain two more identities. A non-cloning
`from_parts` constructor validates all scalar ownership relationships. Only
the outer factory clones once per selected child owner in documented order.
All reservations are fallible, explicit and bounded by the admitted source.

A safe observer consumes appended `[anchor, literal length, match length,
offBase]` records, validates contiguous source spans and actual match bytes,
and reconstructs the exact `OffsetIntent` against the running wire repeat
triple. It appends checked literal bytes and logical sequences, then the final
literal tail. Sequence count and final repeats must equal the parser result.
Argument errors precede mutation; reserve/parser/observer failure poisons the
state. Existing leaf unsafe features change storage only; the copied plan,
observer and state coordination need no artificial unsafe representation.

Static `ReferenceSearchPlan`/`ReferenceSearchState` dispatch will select this
private variant while the public strategy gate remains closed. Shared frame
entropy owns the sole committed repeat state and performs existing raw/RLE/
compressed publication. The initial one-block state accepts only the full
source and initial repeats, so it cannot silently generalize to partial or
retained calls. Public admission follows private complete-frame validation
and an additive, source-size-aware successor of the frozen v5 mappings.
Every newly admitted parameter tuple must be independently reconstructed from
pinned presets and unchanged request/source inputs before interoperability.

### Complete Optimal frame measurement records

A separate public benchmark will measure the complete one-shot frame path only
within the newly qualified source domain. Its immutable corpus records own
64..16,384-byte deterministic vectors; copied request records describe default
levels 11/12 and bounded explicit strategy-7 tuples. Before clocks, it resolves
every request, independently decodes the complete result, fingerprints full
source and frame bytes, and records an allocator ownership ledger. The Global
allocation path alone is timed; corpus construction, checksums, decoding and
tracking-allocator accounting remain outside the interval. Each repetition
compresses and drops a complete result, including all temporary ownership.

All 19 isolated leaf features, safe and all-feature builds must produce equal
full-frame bytes and owner ledgers on identical records before timing begins.
The driver builds every executable before the first measurement and validates
explicit evidence that native correctness, feature and platform jobs completed.
A separately invoked safe/all repeat follows the first campaign. Fixed-order
desktop timings are diagnostic observations, not stable speedup guarantees;
features absent from a particular path must not be claimed to accelerate it.
The benchmark source/requests and byte fingerprints are frozen when first run;
any subsequent correction requires a versioned successor and preserved logs.

### Reference entropy minimum-match admission

The shared reference reconstruction preflight originally described only parsers
with a four-byte minimum. Optimal minimum-3 keys can emit valid three-byte
matches, including zero-literal Repeat3 transitions. A private copied
`ReferenceMinimum` discriminator now selects the format-valid three-byte bound
for the dedicated automatic-literal Optimal entry; the existing reference entry
keeps its four-byte contract. The same checked total, repeat-intent, gain and
transactional publication machinery is reused. Sequence capacity is bounded by
`MAX_BLOCK_SIZE / minimum`, before iteration or allocation. Named-policy entropy
and preceding parser contracts are unchanged. This corresponds to the Zstandard
format's minimum match length and `ZSTD_storeSeq` output, not a new entropy codec.

## Bounded reference Ultra: design before implementation

The next strategy is `ZSTD_btultra` (numeric 8), corresponding to
`ZSTD_compressBlock_opt2` and `ZSTD_compressBlock_opt_generic` with optLevel 2 in
pinned v1.5.7 `lib/compress/zstd_opt.c`. Strategy 9 (`btultra2`) additionally
seeds statistics with a first discarded pass and rewrites history coordinates;
that lifecycle is a separate module. No public strategy-8 admission follows from
merely adding fractional prices.

### Copied policy and scalar prices

A private copied policy identifies the exact coupled whole-bit Optimal or
fractional Ultra behavior. It owns no allocation or pointer and has one safe
representation for every feature configuration. Do not expose independently
selectable price/search shortcuts that upstream never uses together. Existing
Optimal construction defaults, observations and all baseline bytes must remain
identical.

Fractional weight corresponds to `ZSTD_fracWeight`: with adjusted count `n+1`,
base `floor(log2(n+1))*256` and fractional term
`((n+1)<<8)>>floor(log2(n+1))`. Derive and explicitly reject the upstream shift
and intermediate-overflow boundaries before arithmetic. The pinned debug contract
`highbit(n+1)+8<31` restricts raw statistics to 0..8388606 inclusive;
fractional weights consequently range from 256 through 6143. Reject the next
raw value even though an unrestricted u32 left shift could still hold its
intermediate. An independent arithmetic model can compute the fraction by
integer division, while the native oracle retains the original shift order.
Counts and cached bases
must use the same policy. Existing `SymbolPrice` whole-bit divisibility and
31*256 bounds cannot silently be reused for fractional values. A checked scalar
price record should validate its own policy-dependent range and consumed
frequency; no artificial unsafe scalar implementation is appropriate.

Ultra uses fractional predefined LL/ML weights, fractional dynamic symbol bases,
and removes the optLevel<2 large-offset penalty. Preserve the shared dynamic
match-price bias and independently qualify signed literal-length increments.
An integer interpolation model plus direct pinned C observations must precede
Rust implementation; test zeros, powers of two and adjacent values, maximum
legal statistics, literal disabled mode, all code boundaries and overflow
rejections. Preserve distinct C evaluation order where it affects boundaries.

### Frequency ownership and cache lifecycle

Reuse the existing contiguous 377-cell allocator owner and its isolated
`unsafe-optimal-frequencies` access contract. Store immutable copied policy with
the owner; reset clears counters/state without changing policy or allocator
identity. Fresh initialization, scaling, sequence updates and cache refresh
retain their independent invariants. Cached prices cannot be mixed across
policies, and any future state composition must reject a policy mismatch before
mutation or allocation. Existing snapshots/fixtures remain frozen; a successor
observation protocol must carry explicit mode rather than reinterpret old data.

### Path ownership and thorough relaxation

Reuse the initialized 4099-cell path owner, match buffer and index-based reverse
links. Ultra adds the match-plus-one-literal alternative when replacing a match
with literals, retains the previous match's repeat-code history while considering
that alternative, does not skip positions using Optimal's half-bit heuristic,
and does not break descending match-length relaxation after one worse price.
All reachable states remain finite and every predecessor index must be checked
before access. The extra alternative may extend the frontier by one cell; prove
capacity and initialization of both current+1 and last+1 before writing. Replay
complete intermediate paths, repeat arrays, choices and frequencies, not just
final sequence bytes. The existing `unsafe-optimal-paths` feature remains the
only alternate path-memory implementation.

### Complete source and frame ownership

Initially retain the independently qualified no-dictionary complete fresh source
envelope: 64..16384 bytes, source fitting W<=16, H/C<=12, S<=6, minimum3..7 and
target<=4095. Search table geometry and all-match ownership remain separately
qualified. A dedicated immutable execution plan couples strategy, policy and
source identity, checks every layout before allocation, and owns no allocator.
Composition consumes exact non-cloning owners; policy/source/layout mismatches
are rejected before mutation. The shared entropy publication remains the sole
committed frame owner, with three-byte minimum allowed only for a qualified
parser. Dictionary, retained public blocks, window expiry, LDM and Ultra2 remain
outside initial admission.

The new public gate requires frozen source-only requests, independent scalar
resolution, whole-C normal/sanitized observers, complete intermediate-state
replay, allocation-failure cleanup and safe/unsafe equivalence, followed by
native bidirectional complete-frame checks and an unchanged full feature matrix.
Only then define source-aware successor admission classifications. Preserve all
historical selectors, input values, byte bounds and previous fixture checks.

### Ultra scalar observation ownership

Before lifecycle changes, a bounded scalar protocol supplies copied mode and
query words, never pointers. Its independent Python model derives fractional
weights by integer division rather than the C shift sequence. The native helper
owns fixed initialized source/frequency arrays, validates the entire input before
calling upstream, and checks that queries do not mutate its records or guards.
Ordinary frequency lifecycle is deliberately absent: these are labeled synthetic
scalar states. A later Rust fixture reader owns no source table and borrows
bounded packet rows; copied checked policy/price values require no unsafe variant.
All fixtures record mode explicitly and preserve the original whole-bit corpus.

### Checked Ultra price values and compatibility entry points

Extend the private scalar price module with copied `PricePolicy::{Optimal,Ultra}`.
The policy couples the whole-bit mode and fractional mode; later path/frequency
composition must carry this same discriminator rather than independent knobs.
`SymbolPrice` gains a copied policy alongside its finite cached base and weighted
frequency. It owns no memory or source reference, so its representation stays
shared across safe/unsafe features. Existing constructors and query entry points
remain Optimal wrappers with unchanged admission and results.

A policy-aware symbol constructor validates the raw frequency's domain, exact
representability of the cached base, and frequency weight <= base. Whole-bit
bases remain multiples of256 through7936. Fractional bases are256..6143, with
quantization constraints below exponent8: not every integer in the numeric range
is an obtainable base. For base `256*(k+1)+r`, k<8 requires r divisible by
`2^(8-k)`; k>=8 permits every remainder. This is a cached-weight value contract,
not a claim that a synthetic count vector is an ordinary CCtx state.

Every dynamic LL/match query checks that all supplied symbol policies match the
query policy before arithmetic; a mismatch is InvalidStatistics. Literal slices
are borrowed only for a call. Empty literals preserve the no-read zero result;
positive queries validate every consumed frequency and apply the minimum one-bit
cost. Disabled/predefined literals retain shared six/eight-bit behavior. LL and
match length bounds, offset intents, unreachable sentinel and checked additions
retain their established contracts. Ultra predefined LL/ML prices use fractional
weights and dynamic matches omit the large-offset handicap while retaining51.
Signed adjacent LL increments are independently compared by subtracting finite
prices in a wide signed type; no new path admission follows from these scalars.

### Ultra scalar fixture transport

The native-audited scalar fixture uses magic `ZUSFIX01`, little-endian u32 count,
then ten u32 request words and one little-endian signed i64 result per row.
Count is exactly2470 for this frozen corpus; total size118572bytes. Projection
requires full capture audit and normal/instrumented equality before writing any
fixture. Provenance binds original capture, source-only manifest, packet and
both complete transcript hashes. A separate offline checker reconstructs every
row from the independent source-only model and rejects changed extents, requests
and results, even when presented with plausible in-range scalar values.

The Rust test reader borrows immutable bytes, checks exact extents before slicing,
and validates each copied row's mode, operation, reserved words, statistic sum,
length/offset and signed-result domains independently of production price code.
It allocates no table or codec state. Iterator items remain checked copied rows;
no unsafe indexing or lifetime extension is appropriate. Malformed reader tests
cover truncation, appended data, count bounds and invalid query fields.

### Policy-bound frequency lifecycle successor

The next frequency capture adds an explicit copied policy word to the existing
bounded no-dictionary price schedule. Original147requests retain every byte,
operation and source; they are independently wrapped for modes0/2. Whole-mode
model observations must reconstruct the entire immutable original C fixture
before the successor observer runs. A fractional wrapper that exceeds its local
weight bound is an explicit negative case, not a rewritten predecessor request.

For the native successor's conservative preflight bound, compute the first
literal histogram's scaled mass directly from the specified first source span.
Enabled fractional literals require `initial_mass + 2*total_updated_literals <=
8388606` across the whole schedule, even if rescaling later reduces that mass.
This is a documented observation-envelope restriction, not an upstream format
limit. Sequence update budgets already keep their sums far below the fractional
bound. Exact-boundary source-only schedules reach the final allowed mass and
reject the next increment before any C call. Full377-cell tables, sums, cached
prices and query results remain observed after every scheduled operation;
batched updates still invoke C once per primitive update.

Production frequency owners gain an immutable PricePolicy, and snapshots include
that copied discriminator. The existing constructor selects Optimal. An explicit
private policy constructor keeps the same single377-cell allocation and exact
allocator identity. Reset preserves policy and allocator while clearing counts,
phase and caches. Cached zeros while uninitialized are not valid query prices.
Updates validate fractional next sums before publication; rejection leaves counts,
caches, dirty flags and policy unchanged. Snapshot publication rejects a policy
mismatch before any write. Refresh computes all active policy-bound caches before
publishing them, and disabled literal caches remain zero.

Until Ultra's path relaxation is implemented and independently qualified, the
existing Optimal parser's owner-consuming constructor rejects a frequency owner
whose policy is Ultra before resetting frequencies, paths or matches. This avoids
accidentally combining fractional prices with optLevel0 search/path shortcuts.
There is still no public strategy8 admission. The existing isolated frequency
unsafe feature changes only checked cell access; policy and counter arithmetic
remain common safe value operations.

### Complete policy-tagged frequency fixture

The successor binary uses `ZUFRQF01`, a u32 case count, then per-case u32 name
length, source length, operation count, literal mode and policy. Name/source
bytes follow; each operation retains six request words and the same390-word
complete state/query record as the original Optimal fixture. Policy is explicit
in each case and must agree with every audited native record before projection.
The298positive cases contain4065complete observations; both rejected wrappers
remain in the source-only manifest/capture, not in this positive fixture.

An independent reader borrows source/name bytes and fallibly owns per-case
observation vectors; allocations are bounded by the fixed40MiB pack envelope,
298cases and4065observations. All table widths and arithmetic extents are checked
before access. It validates policy, source spans, dirty-cache lifecycle, update
budgets, exact sums, disabled literal state and refreshed/retained cached weights
without production frequency code. Fractional cache reconstruction uses wide
integer division rather than production shifts. The cumulative transport guard
is independently recomputed from the initial source histogram. Immutable original
whole-mode bytes and request values remain separately qualified.

### Policy-bound frequency measurement ownership

A focused frequency harness will reuse the immutable, independently checked
298-case native fixture. Qualification replays every operation and compares all
377 counts, four sums, four cached bases, phase and all query prices outside
clocks. Selected deterministic workloads pair policies 0 and 2 for single-symbol
initialization, full-block cyclic initialization, 8192 symbol updates, disabled
literal symbol queries, grouped dirty updates and the final permitted fractional
literal sum. Original operations and source slices are never rewritten.

The measured unit constructs one Global-owned 377-cell table, replays the exact
schedule, copies its final snapshot and drops the table. Fixture decoding, full
intermediate comparisons, allocator monitoring and output formatting stay outside
timing. Queries and final snapshots pass through black_box; elapsed samples measure
complete lifecycle latency, not compression throughput or isolated unchecked loads.
An untimed exact borrowed/non-cloning allocator pass verifies one allocation,
1508 requested bytes, no growth, no leaked allocation and exact owner release.
Timed final snapshots must equal the previously checked complete state after
each sample; no checksum alone establishes correctness.

The harness reports policy, case name, primitive operation count and sample count.
A controller freezes inputs/source/build commands, builds safe, isolated frequency
unsafe and all-feature executables, completes clock-free qualification and waits
for unrelated Rust/native jobs before timing. A second independent safe/all run
provides a repeat. Raw samples, exact toolchain/environment, complete final states
and owner records are retained; any measured difference is local evidence only.

### Coupled Ultra scripted path replay successor

The next observer isolates path selection using bounded source-owned scripted
match candidates, before ordinary match discovery is integrated. Its request owns
an immutable original wider path request (9..8192 source bytes, at most 64 scripted
positions, at most eight strictly increasing candidate lengths per position) and
a copied policy 0 or 2. Original 17 small and 10 horizon requests retain their
exact source, parameters and candidate bytes. A policy-0 replay using the new
frequency model must reconstruct the complete original 27-case C fixture before
any successor native execution. Scripted repeat candidates remain validated at
the actual callback with the then-current repeat history and literal-zero flag.

The replay owns copied seven-word path cells, canonical complete-cell deltas,
frequency tables and sequence records. Indices identify predecessors and the
frontier; snapshots never contain borrowed upstream pointers. Policy 2 has three
coupled changes: fractional prices, querying positions that policy 0 skips after
a half-bit comparison, and continuing shorter match relaxations after a rejected
longer candidate. A fourth branch preserves a replaced match followed by one
literal when LL(1)-LL(0) is negative. Save the complete replaced match before
publishing the cheaper literal extension. Resolve its repeat history from the
original match predecessor before overwriting the next cell. The alternative
requires strict improvement against both the longer literal run and the existing
next cell; its frontier extension is part of the observable state.

A new `alternative` event will be emitted immediately before the native strict
comparison. It records the saved seven-word match, signed LL increment, candidate
match-plus-literal price, competing longer-run price, existing next-cell price,
and predecessor index. Complete canonical path/frequency state is included as
for every other event. The later `literal` event observes any accepted next-cell
write, copied repeat history and frontier extension. Unique-anchor instrumentation
must preserve all original parser statements; only the match-provider callback
is substituted. Two padding passes and normal/full sanitizer builds must agree.

Source-only recipes must separately witness considered/selected/rejected
alternatives, frontier extension, continued queries, continued descending
relaxation and an improvement after a rejection. These cases are selected using
only the independent model, never observed C output. Native admission and all
public Ultra APIs remain gated until the complete observer, path owner and ordinary
search/frame composition are independently qualified.

### Ultra path fixture and independent reader ownership

The projected fixture uses `ZUPFX01\0`, a u32 case count, and nine u32 fields per
case: name/source lengths, minimum, target, literal mode, path cell count, event
count, script count and policy. All requests use the wider bounded transport;
there is no legacy small/wide discriminator. Name/source bytes and scripted
candidates retain their native request order. Existing stage numbers 0..11 stay
unchanged; stage 12 is the saved-match alternative observation.

Each event retains the prior complete-state delta encoding. After the optional
query and its candidates, an optional alternative flag carries twelve signed
little-endian i64 values. The remaining canonical path/frequency changes and
append-only sequence records retain their original field widths. The pack is
bounded by 32 MiB, 58 cases, 16384 events per case and 65672 total events. Sources
and names are borrowed; scripts, events and change lists use bounded fallible Vec
allocations. One temporary path vector and fixed frequency array reconstruct each
case; the reader never allocates a complete path table per event.

The independent reader recomputes whole/fractional refreshed caches with wide
integer division and preserves dirty caches between refreshes. Before applying
an alternative event's deltas, its saved match must equal the preceding complete
current-cell value. After applying them, recompute the signed LL increment and
both candidate prices from the recorded frequencies/source and current literal
cell. The next-cell price and predecessor index must agree with reconstructed
state. The following literal event must contain exactly the accepted next-cell
write and repeat publication, or no path write for a rejected alternative, with
the corresponding unchanged or extended frontier. These checks are independent
of the production path parser, which remains unimplemented for Ultra.

### Private coupled-policy path implementation

The parser continues to own exactly one frequency owner, path owner and candidate
owner, each retaining its caller allocator. An explicit private policy constructor
checks the supplied immutable frequency policy before resetting any part. Existing
`from_parts` keeps its Optimal contract. Ultra construction is bounded to the
qualified 9..8192-byte scripted envelope; ordinary fresh/retained search and token
planning entry points reject Ultra until their separate integration is qualified.
Policy is read from the frequency owner, avoiding a second potentially inconsistent
policy field. Reset retains that policy and all allocations.

Forward relaxation saves a copied `PathCell` before literal replacement. Ultra
calculates the signed LL(1)-LL(0) difference and both alternative prices using
checked `PathPrice` arithmetic. The observer receives the twelve copied signed
values before the strict comparison. An accepted candidate resolves repeats from
a checked predecessor index and writes the saved match plus one literal into the
next checked path cell, extending the frontier when needed. The half-bit query
skip and first-rejected-shorter-match break apply only to Optimal. Reconstruction,
output validation, poisoning, owner release and phase contracts stay shared.

No new allocation or unsafe operation is needed: the price, saved cell and event
are plain copied values, and existing independently selectable frequency/path/
candidate access backends implement the same storage contract. Verification
compares all 58 native cases, complete paths/frequencies, alternatives, callbacks,
sequences and final repeats. Focused mismatch, bounds, callback-failure/reset and
exact borrowed-owner checks precede the feature/platform qualification item.

### Ordinary Ultra search/path successor

Before enabling ordinary providers, an additive replay will compose the immutable
whole/fractional frequency model and thorough path rules with the already qualified
source-driven tree/short-key search. It owns the search heads, child indices,
short heads and insertion frontiers, as well as persistent path/frequency state.
No native matches or scripted candidates supply model inputs. Search changes are
observed at each actual policy-dependent query; every non-query event preserves
the prior search state. This matters because Ultra's additional queries change
future tree state even though the search routine itself has no optLevel argument.

Requests pair copied policy 0/2 with an immutable original complete fresh request.
Preserve all twenty original 64..16384-byte requests, parameters and source bytes
under policy 0, and require exact reconciliation of every original native event
before new capture. The wider model does not increase the currently admitted
9..8192-byte private scripted Rust bound. Additional source-only recipes must
cover ordinary alternatives, frontier extension and thorough relaxation before
a native observer is reviewed. Strategy 9 and retained contexts remain separate.

Complete search/path deltas retain the original canonical sparse encoding. New
policy and signed-alternative fields are additive. Committed/pending repeat owners
and raw/RLE/compressed publication hypotheses are explicit copied inputs; the
model predicts parser state and publication consequences, not entropy payload
bytes or which block kind the encoder will choose. Native execution later verifies
each hypothesis and all ownership/guard records. Frame bytes remain opaque
observations until independently decoded. Allocation-free Ultra execution planning
and shared-entropy publication follow this ordinary-search qualification.

### Ordinary Ultra native observer and capture ownership

The additive `ultra_fresh.c` adapter retains the complete fresh Optimal CCtx,
allocator, search snapshots, 4099 path cells and 16384-byte source envelope.
It reads one descriptor with `O_NOFOLLOW | O_NONBLOCK`, verifies regular-file
identity and the 124..16444-byte extent using `fstat`, and consumes exactly that
packet before any upstream allocation. The outer policy occupies bytes 8..12;
the immutable nested Optimal packet starts at byte 12 and source data at byte 60.
Both the applied-parameter invariant and CCtx construction select btopt for policy
0 and btultra for policy 2. The ordinary upstream match provider is unchanged.

The original fourteen observation substitutions are augmented by one read-only
saved-match hook. Its twelve signed i64 values are observer-owned copies, reset
after emission and at each padding pass. It receives the current repeat pointer
and short-search frontier so its snapshot has exactly the same complete-state
contract as every other non-query event. Removing all fifteen hooks must recover
the exact pinned source, including the match-provider selector. Policy tags appear
on metadata, events, block begin/publication and completion records.

The command plan owns immutable packet bytes and exact diagnostic strings. Both
normal and ASan/UBSan builds compile all 26 used translation units, with dependency
and effective-predefine gates before helper execution. Captures retain frames,
allocator ledgers, compiler identity, source closure, commands and bounded logs.
The independent auditor regenerates state from original sources and compares
canonical records, not only hashes. It retains transcript digests for cross-build
equality and limits predicted-line caching to two cases to avoid retaining the
entire expanded campaign. Aggregate log storage is bounded by 2 GiB; each valid
request remains bounded by 256 MiB and 65536 events per padding pass. Publication
kinds stay explicit hypotheses until compared with native block headers and owners.

### Ordinary Ultra fixture and independent borrowed reader

The additive fixture uses `ZUFFX01\0`, a u32 case count, and the original complete
fresh twelve-word case header plus one policy word. The policy is 0 or 2. Two
three-word allocation ledgers, borrowed ASCII name/source/frame bytes and the
single twenty-word block publication record follow. Every event retains the
original ten-word header, optional stretch/reverse/query, candidate pairs and
canonical path/frequency/search deltas. An optional twelve-i64 alternative record
is inserted immediately after query candidates, matching the scripted Ultra
fixture's signed representation. Case policy applies to every event; projecting
native records must verify all redundant policy tags before removing them.

The pack is bounded by 64 MiB, 44 cases, 65536 events per case and 198476 total
events. Each source remains 64..16384 bytes inside its window. All 4099 path cells
and complete search owners are reconstructed. Names, source and frame bytes are
borrowed; event/change vectors use bounded fallible allocation. One temporary
path/search state and a fixed frequency array suffice per case. Committed/pending
repeat-owner checks, canonical delta validation, wire frame structure, exact
allocator high-water marks and sanitizer redzones retain their original contract.

The independent reader reuses independently implemented Ultra event and frequency
checks, including fractional cache refresh and unchanged dirty caches. It verifies
the complete saved match before applying alternative deltas, recomputes signed
LL increments and both candidate prices, and checks the following literal event's
exact accepted/rejected publication and frontier. Ordinary non-query events may
not change search; initial frontiers and biased source positions stay checked.
No production parser supplies reader decisions or fixture reconstruction inputs.
Captured frames are separately decoded with the Rust decoder. A source-only
reconstructor must reproduce every state byte and retain all twelve-operand,
search/path/publication, count and transport forgery controls.

### Private ordinary Ultra parser admission

A separate private `from_ultra_fresh_parts` constructor will admit matching Ultra
frequency owners only for 64..16384-byte sources, with the already checked minimum
and target bounds. The existing Optimal constructor and 9..8192-byte scripted
Ultra constructor retain their rejection contracts. Shared initialization runs
only after constructor-specific policy/source checks, and resets the same three
moved owners. No allocation, unsafe access or alternate search implementation is
introduced. Borrowed non-cloning allocator identities remain with each owner.

A separate `parse_ultra_fresh` entry validates the Ultra policy, source envelope,
source identity and complete fresh search geometry/state before mutation, then
uses the same ordinary provider and thorough coupled-policy path engine. Existing
Optimal fresh, retained and execution-planning gates remain unchanged. Parser and
search must both be reset after failure; observer effects remain caller-owned.
The private ordinary entry does not publish entropy tokens or enable strategy 8.

Qualification compares all path/frequency/query/alternative/sequence events and
final repeats, then independently replays the same query schedule against every
complete search owner before and after every query and non-query event. Policy-0
baselines retain all original requests and complete frames. Focused controls cover
foreign source, mismatched parameters, used search, owner policy/source bounds,
every moved-owner allocation failure, borrowed-owner parse/reset allocation
counts, alternative-observer poisoning and successful complete replay after reset.

A copied `ultra_fresh_only` admission bit prevents using the larger ordinary
constructor with an arbitrary scripted provider. Generic scripted parsing rejects
that owner before changing status. The ordinary entry calls the shared private
runner after source/search validation. Reset retains this bit; it is a plain value
record shared by all access backends and needs no artificial unsafe implementation.

### Private Ultra execution and token composition

`OptimalExecutionPlan` remains a plain copied record. Its new private
`from_ultra_parameters` entry admits only resolved strategy 8 and delegates to
checked geometry/layout arithmetic shared with strategy 7. The original entry
continues to reject strategy 8. Both use the already observed 64..16384-byte
fresh source domain, W16/H12/C12/S6 and target4095 caps, complete-source windows,
nine distinct owner byte counts and a checked raw-frame bound. No allocations,
allocator clones, native calls or unsafe representation are involved. Upstream
counterparts are `ZSTD_getCParams_internal` / adjusted compression parameters and
`ZSTD_compressBlock_btultra`; the local resource envelope is not a format limit.

`OptimalMatchState` gains a separately checked Ultra constructor. The plan
strategy and unused ordinary Ultra parser admission must agree before any scratch
is cleared. Its seven parser/search owners and two scratch owners retain exact
allocator identities. Source pointer/length, effective minimum, target, geometry,
literal policy and unused search/parser state are checked together. The source
is borrowed and tokens own only literal bytes and `SequenceWithOffset` records.
The existing Optimal constructor rejects a forged Ultra plan even if paired with
an Optimal parser. A shared collector preserves checked three-byte minimum,
wire offset intent, overlap reconstruction, repeat evolution and literal tail.
Reservation failures poison the composite; reset clears both parser/search and
scratch while preserving capacities and policy. No new storage backend is needed.

Private `ReferenceSearchPlan::Ultra` selects the same statically stored token
state, with a distinct plan variant to prevent accidental public admission. The
factory validates variant/strategy correspondence before cloning any owner. The
exact nine clones remain frequencies, paths, parser candidates, tree heads, tree
edges, short heads, search candidates, literals and sequences. No inactive parser
or additional dispatch allocation is introduced. The historical request selector
continues to reject strategy 8 until separate source-aware admission qualifies.

The shared frame workspace accepts only one full source block for either optimal
strategy and passes checked tokens to the existing three-byte entropy contract.
Committed entropy/repeats stay in the entropy child. Candidate parsing does not
commit wire state; compressed publication commits it, raw fallback discards it,
and any failure after preflight poisons the workspace. Invalid partial scheduling
is transactional. Rust entropy bytes need not equal C; complete frames must be
deterministic and decode exactly, with native bidirectional interoperability a
separate gate. Raw/disabled-literal observation cases are not silently admitted
as public literal-policy controls.

### Source-aware Ultra admission successor

An additive v7 classification projects all historical v6 rows unchanged, retains
all original columns/source/request identities, and independently reconstructs
all 27 strategy-8 tuples from the pinned preset/resolution source model. Only the
new v7 columns classify strategy 8 as success/resource within the qualified
ordinary domain. Strategy 9 stays unsupported. The projection cannot execute
children. Its malformed/type/geometry controls run under normal and optimized
Python before any public dispatch change. New native public recipes preserve all
1269 predecessor requests and frames as a baseline and add ordinary observed
Ultra sources, default/explicit equivalences and exact resource boundaries.

## Bounded reference Ultra2: design before implementation

Pinned v1.5.7 `ZSTD_compressBlock_btultra2` uses the already coupled optLevel2
parser twice only when the original block is eligible. `ZSTD_initStats_ultra`
collects the first pass into the same frequency owner, discards its tokens and
repeat changes, invalidates the old history range and starts the real pass with
scaled accumulated statistics. Strategy 9 must remain unsupported until this
complete lifecycle is independently observed and qualified.

### Coordinates, phases and ownership

A private copied phase (`Fresh`, `Seeding`, `Rebased`, `Encoding`, `Complete`,
`Failed`) owns no memory and needs no unsafe representation. It must encode
which transitions are legal and whether output is publishable. A failed pass
poisons the complete coordinator; only an explicit whole reset may reuse it.
Caller-visible repeats, tokens and frame bytes remain unchanged during seeding.
The borrowed source retains its ordinary lifetime throughout both scans.

A checked coordinate record stores the absolute index origin, low/dictionary
limit, update frontier and source length as integers. The C implementation
moves `window.base` backwards by `srcSize` and advances `dictLimit`, `lowLimit`
and `nextToUpdate` to invalidate old positions. Rust must never form a pointer
before the source allocation. Translate eligible positions through checked
subtraction and explicit interval validation; stale first-pass tree and hash
cells remain integer records and cannot become dereferenceable history.
Whether clearing all old cells is observationally equivalent must be proven
against complete native intermediate state before selecting that simplification.
The short-key table, tree edges/heads and candidate frontier need the same
coordinate proof. Ring-slot aliases after rebasing require explicit coverage.

The existing frequency, path, match-buffer and search owners can be reused if
their phase-reset contracts support the exact upstream transitions. The first
pass must preserve all collected frequency counters; the second call to
`ZSTD_rescaleFreqs` scales those counters and recomputes cached bases. Clearing
frequency state, keeping stale caches or calling fresh histogram initialization
again changes the algorithm. Path/candidate scratch may be reset without
retaining externally observable choices only after that equivalence is checked.
A discarded token sink should borrow existing literal/sequence buffers and
reset their logical lengths, retaining the same allocating owner and capacities.
Do not clone owners merely to discard the first pass. The real pass starts with
the original repeat offsets, since C gives the initial pass a local `tmpRep`.

### Eligibility and qualification boundaries

The native gate tests all of: zero initial literal-length sum, an empty sequence
store, equal dictionary and low limits, current position equal to dictionary
limit, and source length greater than `ZSTD_PREDEF_THRESHOLD` (8). Each
predicate and the 7/8/9 boundaries need independent controls. Inputs
that skip seeding still run the ordinary optLevel2 parser; they must match Ultra
when source, parameters and initial state are identical. Existing Ultra/Optimal
source requests and complete frames remain frozen.

The initial 64-byte public lower bound means every otherwise eligible public
request exceeds the native threshold; the smaller threshold controls belong to
the private model/observer, without widening public admission.

Initially reuse the complete fresh no-dictionary 64..16384 source envelope and
existing geometry caps. Keep dictionary, prefix, LDM, retained public blocks,
window expiry and multiple-block compression outside public admission. Native
observer transport must expose pass identity, original/copied repeats, frequency
and cache state before/after scaling, sequence-store reset, all coordinate
transitions and complete search/path events. Instrumentation is read-only; the
original C initial-pass call, coordinate arithmetic and predicates stay intact.
The independent model must derive both passes before native execution, including
cases that differ from ordinary Ultra, to prevent final-frame-only validation.

### Exact first-pass reset boundaries

`nextToUpdate3` is a local variable initialized from `ms->nextToUpdate` on each
`ZSTD_compressBlock_opt_generic` invocation; it is not an independently retained
match-state field. The second pass therefore initializes it from the rebased
frontier, while the existing hash3 cells themselves survive as stale indices.
`lastStretch` is zero-initialized each invocation, and `ip` skips the first byte
when it equals `prefixStart` in both passes. The seeding call's returned tail
length is ignored; the helper must not invent a final-literal statistics update
or an entropy publication between passes. `ZSTD_resetSeqStore` resets logical
sequence/literal output before the real pass. The observer must distinguish
physical stale cells from readable current history and report both where the
native state exposes them.

### Independent Ultra2 eligibility and source view

The source-only `Entry` record keeps the five native eligibility predicates
separate from the helper's unsigned `dictLimit - nextToUpdate <= 1` assertion.
An invalid update frontier is not silently reclassified as a sixth skip predicate.
Checked `Coordinates` map absolute indices into one source interval; rebasing
advances the origin by the complete source length and rejects overflow of the
second interval. Every byte in the first interval is then stale, even if its
integer index aliases a current tree ring slot. Empty ranges may name the end;
positive reads may not. These copied values own no allocation and require no
unsafe representation.

The independent two-pass search model retains the actual hash/tree/short-key
lists while replacing only its checked source view and update frontiers. The
view presents the absolute coordinate extent expected by the already qualified
ordinary model but rejects access to its nonexistent prefix; it allocates no
fake history bytes. Ordinary path offsets stay relative to the same source.
Both scans retain the same frequency and path owners, while each parser invocation
starts with original repeats and an empty logical sequence list. Per-pass copied
observation rows own their snapshots, so later updates cannot rewrite earlier
predictions. Model-only source-read counters are excluded from the native wire
protocol rather than presented as native observations.

Skipped-seeding reconciliation initially uses only a nonempty opaque sequence
prefix with otherwise fresh parser state. The sequence payload is outside that
model's contract. Other failed gate predicates are checked as independent
eligibility arithmetic; they are not claimed to reproduce dictionary/retained
parser states or an already observed native Ultra2 wrapper.

### Two-pass observation storage and transport

`ZU2PS01\0` owns exactly 44 bytes of header plus 7..16384 source bytes. Its nine
little-endian u32 fields are source size, seed hypothesis (0/1), window/hash/chain/
search logs, minimum match, target and literal mode. One entire block is implicit;
7/8/9-byte threshold cases are private controls, not a public lower-bound change.
The parser rejects size, arithmetic, geometry, mode and trailing-byte errors
before allocation or native calls. Request records own bytes; frame-independent
observation readers borrow bounded lines and copy validated state only as needed.

The observer will use a real CCtx allocation with its original frequency, path,
match, search and sequence owners, but directly invoke the unchanged private
`ZSTD_compressBlock_btultra2` wrapper. No entropy selection or frame publication
occurs in this lifecycle oracle. A leading source guard of at least one complete
source length keeps the C helper's backwards base movement inside the same
allocated source object. The Rust design still uses integers only.

Pass-tagged observations include eligibility, pre-rescale state, all ordinary
search/path/frequency events, final candidate repeats/tokens, and the original
sequence-store reset plus coordinate transition. Old physical cells remain
observable; only logical output lengths reset. The independent normal/sanitized
builds and two padding fills must agree on every canonical state, source bytes,
guards and released allocation ledger. Seeding-mode zero installs one explicit
zero-literal sentinel sequence before the wrapper to test the native empty-store
predicate; its fields and logical prefix must remain unchanged. Returned parser
observations describe only appended sequences, not that sentinel as codec output.

### Two-pass fixture ownership (planned before projection)

The future `ZU2FX01\0` pack will own request/source bytes, exact native allocator
ledgers, pass/reset descriptors and complete canonical event deltas. `Case<'a>`
will borrow its name and source from the immutable pack; bounded owned vectors
will hold decoded pass records and event/delta lists for tests. No source frame
is present because this observer does not run entropy publication. Original
Ultra packs remain unchanged. These test-only value records have no distinct
unsafe representation and do not add a production allocator owner.

A case header binds geometry, seed mode, one source and one or two algorithm
passes. Each pass binds its phase, absolute origin, original incoming repeats,
pre-rescale frequency state, candidate repeats, final tail and event extent.
A seeded case additionally binds the native reset's discarded counts/repeats,
unchanged caller repeats/frequencies and all advanced history limits. Path and
search sparse baselines persist across passes; frequencies also persist while
logical sequence counters and block-relative wire reconstruction restart.
Resetting the decoder's physical sparse baselines would lose evidence and is
forbidden. Source offsets remain relative, while search indices remain absolute.

The reader must check extents and count arithmetic before reserving memory,
reject noncanonical or duplicate deltas, validate 7/8/9 threshold/phase selection,
and distinguish stale stored indices from readable current source. The second
pass's live interval is `[source_len + 2, 2 * source_len + 2)` in native indices;
old positive table entries may remain below it. A table value in that old range
is not itself malformed, but a match/source read from it is. Ring-slot equality
does not change that distinction. New source translation, geometry and phase
records remain copied integers, with no before-allocation pointers.

Every pass starts wire reconstruction with repeats 1/4/8. The first pass's tokens
are independently checked against the source and then discarded; its final tail
is not a frequency update. The second pre-rescale frequency snapshot must equal
the first final snapshot and reset snapshot. The following begin event must
independently satisfy the preserved-statistics rescale/cache rules. Threshold
controls and skipped-seeding append observations remain private test cases and
must not widen any public admission boundary.

### Private Rust Ultra2 plan and phase transition contract

The private `Ultra2ExecutionPlan` will be a copied, allocation-free value holding
resolved strategy-9 parameters, the checked ordinary search geometry, source
length, literal-price mode, explicit seed hypothesis and nine exact owner sizes.
It admits the private observed 7..16384-byte envelope; no frame settings or public
dispatch are attached to it. The existing Optimal/Ultra plans and their 64-byte
lower bounds remain unchanged. Public Ultra2 admission, when separately qualified,
must still apply its own source-aware 64-byte boundary. The false seed hypothesis
models appended parser observations under an opaque sequence prefix, not a
published preexisting token stream.

`Ultra2Coordinates` will use Rust source indices without the native bias of two:
first origin zero, second origin source length. Checked interval construction
reserves room for the native bias and rejects overflow before forming an end.
`absolute(offset)`, `offset(index, width)` and `rebase()` validate complete ranges;
zero-width end ranges are valid, positive end reads and every stale old index are
not. The original source borrow is the only readable byte storage. `OptimalSource`
will carry this origin and translate all key/hash/byte/match operations through
checked subtraction. Its existing zero-origin constructors and direct-query
contracts stay unchanged. A dedicated private constructor handles threshold
controls without broadening ordinary public entry points.

The search owner retains all physical tree/short-key cells on phase reset and
changes only its source origin, main frontier, block-local short frontier and
logical candidate length. Candidate filtering takes the maximum of the window
floor and the current origin before any source access. Tree ring indices remain
absolute Rust positions plus the native bias, so old physical aliases survive
exactly as observed. The second phase presents relative queries to the parser
and translates them once at the provider boundary; path and token offsets never
change coordinate systems. Source argument errors precede owner mutation.

A copied `Ultra2Phase` transition record distinguishes Ready, Seeding, Rebased,
Encoding, Complete and Failed. Only a legal finished seeding pass can enter the
rebased state. The parser preserves frequency/path allocations and cell contents,
but resets invocation-local anchor, cursor, repeats, last-stretch, sequence count
and candidate length. It must not call the existing whole-parser reset at this
boundary. Re-running `begin_block` performs the observed retained-frequency
scaling. Tiny threshold cases perform begin/complete without a search query;
loop bounds must use checked or saturating subtraction rather than underflow.

The future coordinator owns the same nine allocator objects as ordinary Ultra:
frequency, path, parser matches, tree heads, tree nodes, short keys, search matches,
literals and sequences. Both token vectors reserve once before the first pass.
A private observer collects seed tokens into those same buffers, clears only
logical lengths at reset, and restarts original repeats for encoding. No tail
statistics or entropy publication occurs between passes. Accessors expose no
seed or partial-failure output; failures after work starts poison the coordinator,
and a whole reset is required for reuse. Parameter rejection remains transactional.

### Source-aware public Ultra2 plan and frame boundary

The additive admission-v8 projection will retain all predecessor TSV columns,
request bytes, source selectors and independent tuples. Only resolved strategy 9
changes classification: a complete fresh source of 64..16384 bytes must fit its
window and the qualified W10..16, H/C6..12, S1..6, minimum3..7, target0..4095
geometry. Every historical strategy-9 tuple is independently resolved from the
pinned preset table and original request before classification. Source-only
public recipes retain all 1345 predecessor pairs and add observed seeded sources,
short-key ring residues, changed final paths, default levels 16/17, explicit
aliases and resource boundaries. These hypotheses do not enable execution.

A separate copied `Ultra2FramePlan` will wrap the private execution plan plus
checked `ReferenceFrameSettings` and the raw frame bound. It always requests
seeding and automatic literal prices; the private 7/8/9 controls and opaque
prefix/skip hypothesis cannot escape through that public constructor. Admission
must finish before cloning the allocator factory or reserving any frame owner.
The public static-dispatch variant will construct the same nine child owners in
frequency/path/parser-candidate/tree-head/tree-edge/short-head/search-candidate/
literal/sequence order. The existing frame workspace remains the sole entropy
and committed repeat owner and receives only the completed second-pass tokens.
No first-pass tail statistics or entropy tables are published. The new variant
adds no boxed owner, allocator erasure or unsafe dispatch representation.

Both native C builds must decode every successful safe/all Rust complete frame;
both Rust builds must decode normal/sanitized C frames. Rust safe/all bytes and
normal/sanitized C bytes are compared within their own implementation pairs.
Every previously admitted request must retain its predecessor complete Rust
response, source and resolved tuple. A successor fixture may reclassify an old
strategy-9 rejection; it may not rewrite the predecessor evidence. Public
admission follows private lifecycle, feature, foreign Miri and no-atomics gates.

## Bounded retained Optimal complete frames (Module 20 design)

The existing private `RetainedOptimal` admission is 128..16384 total bytes,
with two to four blocks of 64..8192 bytes. Four schedule slots do not admit a
32768-byte source. A separate additive integer model, pinned native observer,
complete-state fixture and Rust reader must qualify the larger source domain
before changing any production bound. The proposed public extension is
16385..32768 bytes fitting its selected window; all existing fresh 64..16384
requests retain their dispatcher, source resolution and complete frame bytes.
Retained Ultra/Ultra2, dictionaries, prefix attachment, window expiry, numeric
streaming, long-distance matching and workers remain separate work.

### Copied schedule and exact frame reservation

A checked plain-value schedule stores at most four lengths and cumulative ends.
For the proposed new length n, k=ceil(n/8192), q=n/k and r=n%k; the first r
blocks have q+1 bytes and the remaining blocks q bytes. Thus 16385 maps to
[5462,5462,5461], 24576 to [8192,8192,8192], 24577 to
[6145,6144,6144,6144], and 32768 to [8192,8192,8192,8192]. The constructor
validates the complete source/window/geometry and exact checked sum before
cloning an allocator. The next-block accessor checks the expected emitted
position. Setting the existing constant block limit to 8192 is insufficient:
it would produce a one-byte final block at the lower boundary.

The frame plan owns no allocation and supplies the exact checked raw upper
bound: frame-header bytes plus source bytes plus three bytes per scheduled
block, plus any enabled trailer. The current window-based bound counts fewer
headers for these sources. Although per-block fallible reservation is memory
safe, relying on it would violate a complete up-front reservation contract.
All fallback kinds must fit the initial capacity without output growth. Existing
fresh variants keep their current bounds and allocator baselines.

### Retained parser, block-local tokens and publication

The existing coordinator owns seven allocator values for parser and search
storage. Two token scratch vectors bring the complete retained token stage to
nine owners. It retains frequency/path/search allocations across blocks, with
copied initial/current repeat records, source cursor, fixed schedule, pending
`PathResult` and Ready/Pending/Finished/Poisoned phase. Those copied records need
no artificial unsafe representation; existing storage owners retain their
separately selected safe/unsafe implementations and public contracts.

A new collector borrows an explicitly checked absolute block range. Its anchor
and final tail are block-local; tail length is block-end minus anchor, not
whole-source length minus anchor. Matches may reference the consumed prefix,
while future backing bytes remain inaccessible through the active source view.
The existing whole-source collector remains unchanged. Scratch reserves before
parsing and transitions between blocks allocate nothing.

Pinned `ZSTD_compressBlock_opt_generic` reinitializes invocation-local search
frontiers, last-stretch and pricing scale while retaining owners and learned
frequencies. `ZSTD_blockState_confirmRepcodesAndEntropyTables` publishes the
pending compressed-state owner only for compressed output. The Rust frame
workspace is the committed entropy and wire-repeat authority. A bridge validates
pending phase and candidate repeats before appending a complete block, then
commits without a fallible transition. Raw/RLE fallback advances parsing history
but preserves committed repeats. A failure after irreversible parser/search work
poisons the coordinator; no usable state may disagree with already emitted wire
repeats. Complete reset is required for reuse after such a failure.

### Independent larger-domain evidence

The additive transport admits 128..32768 bytes to reconcile all seven earlier
wide native cases unchanged before covering the new domain. It retains the
same two-to-four block schedule, 4099 path cells, bounded search geometry,
32768-event per-block limit, exact sparse deltas and guarded two-padding-pass
observations. A new protocol identity distinguishes the larger packet/frame
bounds. Scalar path/search/price transitions may reuse immutable qualified
integer-model components; captured matches must never become provider inputs.
New source-only recipes and publication hypotheses are frozen before C execution.
The observer needs a source-sized frame buffer with explicit header allowance;
the old fixed 32768-byte frame buffer cannot hold a worst-case new raw frame.

C and Rust entropy encoders may choose different block kinds, which legitimately
changes the next block's committed repeats. Exact native fixtures bind their
captured publication schedule. Complete Rust frames instead require an
independent wire-kind reader and replay of the frozen integer model under their
actual publication history, plus native decoding. Never change the native match
provider or publication to force agreement. Interoperability alone is not an
exact parser-state comparison. Dictionary/window/streaming support is not inferred
from retained history within this bounded complete-source contract.

Reviewed upstream: v1.5.7 revision
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`, `zstd_opt.c` lines 1077..1127
and `zstd_compress.c` lines 3607..3611. These are design hypotheses, not a new
production admission or completed larger-domain qualification.

### Retained execution plan and block scratch refinement

`RetainedOptimalPlan` will be a copied record of resolved Optimal parameters,
source length, checked search layout, at most four block lengths, literal-price
policy, nine independent owner-byte counts and the exact raw frame bound. Its
private constructor accepts only the independently observed retained domain and
explicit checked schedules. A separate frame constructor fixes automatic literal
prices and derives the balanced schedule for 16385..32768 bytes. Neither admits
retained Ultra/Ultra2 or accepts caller wire metadata as scheduling authority.
Both constructors finish all validation and size arithmetic before any allocator
clone. Checked next-length lookup accepts only a scheduled boundary and rejects
an interior offset or completed source.

Literal scratch reserves the largest scheduled block, and sequence scratch at
most floor(largest-block/3) entries, once before parsing. The seven parser/search
owners are unchanged, giving nine total independently tracked allocator owners.
The block collector borrows only the consumed prefix, starts at the absolute
block offset, initializes wire repeats from the committed coordinator and checks
all emitted literal/match ranges against the active end. It hides partially
collected tokens after failure and checks the final tail against that end.
Private literal-disabled plans qualify the captured parser controls; the eventual
public frame constructor cannot expose that experimental policy.

The workspace remains responsible for choosing the actual wire kind. A new
publication bridge must validate pending repeats before output append. Complete
publication and failure poisoning must keep retained repeats consistent with
committed entropy whenever the workspace remains usable. The private coordinator
already records pending candidates and preserves frequencies/search across raw
and RLE fallback; its larger-source qualification does not itself enable public
frame dispatch. Existing fresh collectors and all previously admitted plans keep
their source bounds, bytes and allocator baselines.

The existing `Collect` already checks absolute anchors, overlapping match bytes
and tail length against its borrowed slice. A separate fallible block constructor
can therefore reuse those verified transitions by borrowing `source[..block_end]`
and supplying the checked block start and committed repeats. Its existing fresh
constructor remains unchanged. The new constructor must reject invalid ranges,
repeat records, nonempty scratch and insufficient reserved capacity before
constructing a collector; it neither allocates nor expands the borrowed prefix.
This avoids duplicating offset-intent logic while preserving the original
whole-source contract and making retained initialization explicit.

### Module 20 shared frame publication bridge

The private `ReferenceSearchPlan::RetainedOptimal` variant carries the checked
schedule and exact complete-frame bound. Public request resolution continues to
select only the old fresh Optimal plan until independent wire-history and
platform gates pass. The frame loop asks the plan for its next block; manual
retained workspace calls must match the exact next boundary before mutation.
The selected search factory creates the same nine owners in the qualified order.

Before appending a retained block, the sequence encoder exposes a copied checked
pending repeat snapshot. The frame bridge validates that snapshot against the
parser candidate for compressed publication, or the committed snapshot for raw
and RLE publication. After the complete append it publishes both states without
allocation. Any unexpected failure poisons the enclosing workspace and discards
pending entropy. Old variants have no retained publication callback and preserve
their byte/owner behavior. Reset of this private retained workspace clears parser
history and entropy availability together, retaining every allocation. A copied
pending repeat value needs no alternate unsafe representation.

### Retained Optimal complete-frame measurement records

The benchmark owns 26 fixed source/request/frame records from the previously
qualified `ZORB0001` pack. The Rust example is byte-identical to the frozen
clock-free draft. Its copied parameter records and bounded reader use safe code
for every backend; the selected codec storage features alone choose unsafe
implementations. The Python checker independently binds complete SHA-256 source
and frame identities to the immutable native capture and source-only recipes.

Campaign records own exact command/environment/log identities, complete frame
files, executable hashes, allocation ledgers and five samples per timed case.
Allocation tracking, decoding and byte comparisons run outside clocks. The
controller builds every requested executable before running any of them. Timing
requires completed public regression, private frame platform, native interop and
all-backend clock-free receipts, bound to the current production source. Private
platform-to-public admission changes must be recorded explicitly in the receipt;
interpreted foreign checks do not constitute native foreign execution. An
independent safe/all repeat uses fresh executables and the same frozen inputs.

## Bounded retained Ultra complete frames (Module 21 design)

This design follows the bounded retained Optimal milestone. It proposes an
additive retained strategy-8 path for known 16385..32768-byte sources fitting the
resolved window. Existing fresh Ultra requests through 16384 bytes retain their
current resolution, constructor, allocation contract and exact frame bytes.
Window logs 10..16, hash/chain logs 6..12, search logs 1..6, minimum-match settings
3..7 and target lengths 0..4095 remain the intended resource envelope. The
proposed private observation domain is 128..32768 bytes with two to four blocks
of 64..8192 bytes; its broader schedule controls do not become public options.
The public schedule remains the balanced Module 20 schedule, with complete raw
output capacity checked before allocator cloning. Automatic literal prices are
the only proposed public policy; literal-disabled cases remain private controls.
Retained Ultra2, dictionaries/prefixes, window expiry, numeric streaming,
long-distance matching and workers remain separate scopes.

### Policy-bound ownership and lifecycle

Pinned `ZSTD_compressBlock_btopt` and `ZSTD_compressBlock_btultra` select
`ZSTD_compressBlock_opt_generic` policies 0 and 2 respectively. Retained Ultra
uses policy 2 for every scheduled block, including the first. It does not run the
separate Ultra2 first-block statistics bootstrap. Frequency scaling and search
frontier initialization occur at each block boundary while frequency and search
storage survive publication. Existing whole-bit policy-0 behavior is unchanged.

A checked copied plan binds strategy, price policy, source extent, schedule,
search geometry, exact raw frame bound and nine owner byte counts. A dedicated
retained-Ultra constructor/scope must distinguish this admission from scripted
Ultra, ordinary fresh Ultra and Ultra2. Removing `require_ordinary_policy` without
replacing that admission boundary would accidentally permit the old constructors
to enter a previously unsupported lifecycle. The existing Optimal constructor
must continue to require policy 0; fresh and scripted contracts stay explicit.
Any shared private lifecycle helper receives an already checked scope and cannot
infer retention merely from a source length or policy tag.

The coordinator owns the same seven independently allocated parser/search owners:
frequency cells, path cells, parser match scratch, hash heads, binary-tree links,
optional three-byte hash heads and search match scratch. Two token vectors own
block literals and offset-intent sequences, giving nine owners before the
separately accounted frame/entropy/output workspace. Copied schedule entries,
price-policy tags, repeat records and phase values have one safe representation;
they require no artificial unsafe alternative. Each performance-sensitive owner
continues to select its equivalent safe/unsafe implementation through its own
existing Cargo feature. No new unsafe access is justified by policy selection.

The immutable source borrow spans the full planned input, but each search and
collector view exposes only the consumed prefix through the checked active block
end. Positions and tree links remain indices. Cross-block matches may reference
prior bytes; they may not inspect future backing bytes. Path/search sparse
observation baselines persist across blocks. Initial/current repeats, pending
`PathResult`, schedule index and Ready/Pending/Finished/Poisoned phase remain
copied coordinator state. Policy-2 alternative paths use the retained block's
absolute source position while path indices remain local to the current stretch.

The workspace remains the authority for committed wire repeats and entropy.
Parsing retains learned prices/search for every wire kind. Compressed output
commits pending repeats and entropy together; raw/RLE output discards candidate
repeat publication while retaining parser history. Pending phase/repeat agreement
is checked before appending a block. No fallible owner allocation occurs during
block transitions. Failed parsing or publication poisons the usable state, and
complete reset clears history and entropy availability while retaining storage.
Boundary misuse, observer failure and allocator failures must preserve the
existing early-rejection and no-partial-token exposure contracts.

### Additive independent model and evidence

A new immutable request wraps the exact existing `ZOPRS03` packet plus an exact
integer policy 0 or 2 under a distinct protocol identity. It rejects booleans,
unknown policies, mutable sources/schedules, truncated or trailing bytes and
invalid nested geometry before replay. Earlier requests, packets and captures
remain immutable. Policy 0 must reproduce every retained predecessor state and
publication record after removing only declared additive policy/alternative tags
and restoring the predecessor metadata identity; no observed value may be
normalized away. Policy 2 requires new source-derived qualification.

The successor composes the qualified retained-wide block initializer with the
qualified Ultra path relaxation and policy-bound frequency model. It must not
inherit the ordinary fresh initializer, which requires block zero at source
position zero. Searches are independently derived from the active consumed
prefix. Captured match candidates, path decisions and publication outcomes are
observations, never provider inputs. Source-only publication hypotheses are
explicit request inputs for model tests; they do not predict native or Rust
entropy block choices. Native captures and actual Rust wire histories each need
replay using their own observed block kinds.

Freeze source recipes, expected admission changes, observer adaptation, guarded
padding passes, dependency closure, sanitizer commands and bounded outputs before
new native execution. Cover retained learned-price transitions, short-key history,
policy-2 alternative selection/rejection, horizon/source-end exits and raw/RLE
rollback followed by compressed publication. Qualify malformed schedules, policy
and sparse-state forgeries in ordinary and optimized Python. Every new native
state must reconcile independently before fixture projection or Rust admission.
Preserve the existing observer/capture/auditor helpers; executed successors also
become immutable and failures remain part of the evidence chain.

### Public predecessor compatibility

The current 1513-request public inventory contains one intended transition:
`retained-resource-ultra` (32768 bytes, W15/H12/C12/S6, minimum 3, target 4095)
is currently resource-limited solely because retained Ultra is unsupported. The
proposed extension should admit that exact request; preserve its old rejection as
historical evidence and declare the new expectation before execution. The
`ultra-resource-source-16385` and `ultra-resource-source-32768` requests use W14
and must remain resource-limited because their complete sources exceed the
window. All other predecessor request bytes and response bytes must remain
unchanged. The corresponding early-rejection integration assertion is revised
only at the separately qualified public-admission step, with replacement window,
Ultra2 and over-bound controls retaining exact allocator noninteraction checks.

Reviewed pinned upstream revision: `f8745da6ff1ad1e7bab384bd1f9d742439278e99`.
The studied `lib/compress/zstd_opt.c` SHA-256 is
`625936ee3fb02d789abb894c1fbc2918ee47582434094c963b630da13532c876`, already
bound by the predecessor native dependency inventory. These are design
requirements, not evidence of implemented or admitted retained Ultra support.

### Retained Ultra fixture ownership (before projection)

The additive `ZURFX01\0` fixture preserves the complete ordinary Ultra record
layout under a distinct identity. It has at most 42 cases, 207635 events and
64 MiB total bytes. Each case has thirteen u32 header fields, two three-word
native allocation ledgers, borrowed ASCII name/source/frame bytes and 2..4
contiguous blocks. Sources are 128..32768 bytes and fit the resolved window;
each block is 64..8192 bytes. Headers bind exact policy 0/2, geometry, source,
frame and path-owner extents. Earlier `ZUFFX01` and retained Optimal packs remain
immutable. Native allocation sizes and entropy frame bytes are audited opaque
inputs, not independent model predictions.

A twenty-word block record binds absolute start, length, kind, prior and pending
repeat owners, entry frontier, event count and trailing literals. Event records
retain ordinary Ultra's optional twelve signed 64-bit alternative values,
complete scalar coordinates, query candidates, append-only sequences and sparse
path/frequency/search changes. The 4099 path cells, 386 frequency cells and
bounded search owner persist across blocks. Sequence counts reset at each block;
path, price and search baselines do not. Only compressed publication changes the
committed repeat owner. The native counterpart is the unchanged pinned
`ZSTD_compressBlock_opt_generic` state observed at block entry, event hooks and
publication, not a serialized C pointer layout.

Projection first rechecks the audited capture and emits observed state deltas.
A separate checker reconstructs every state byte solely from source recipes and
independent policy replay; it consumes native frame/ledger bytes only after exact
capture identities and shape checks. Its negative controls bypass the root pack
digest and forge later-block alternative columns, publication owners, search and
frequency deltas, schedule coordinates and truncation. All altered byte strings
must actually differ from the baseline. Ordinary and optimized Python must agree.

The later test-only Rust reader borrows source, name and frame slices. It owns
bounded fallible vectors for cases, blocks, events and changes, plus temporary
path/frequency/search reconstructions that survive block transitions. Checked
extents precede indexing and allocation. It verifies contiguous coverage,
consumed-prefix matches, canonical sparse changes, alternative decisions and
committed/pending publication state without invoking the production parser.
Plain records and the checked test-only reader share one safe representation;
there is no performance-sensitive codec owner needing an artificial unsafe
variant. Frame decoding still exercises safe and each codec backend separately.
Old policy-0 requests, frames, ledgers and every complete state must reconcile
with the immutable retained Optimal fixture before parser admission.


### Retained Ultra constructor scope before execution admission

Replace the parser's `ultra_fresh_only` boolean with the existing copied `Scope`
enum and add `UltraRetained` as an explicit fourth case. This shared plain value
has no allocation or performance-sensitive unsafe representation. The new private
constructor accepts only policy 2 and 128..32768-byte sources, preserving the
existing minimum/target validation and moved frequency/path/match owners. It
resets those three owners only after complete constructor validation, and reset
preserves scope without allocating. Geometry/window/schedule binding remains the
next checked-plan gate; construction alone grants no provider or public entry.

All existing parse entries must reject this new scope before observer/provider
calls or search mutation. Ordinary Optimal retained policy checks stay intact.
The ordinary Ultra configuration accessor requires exactly `UltraFresh`; its
existing scripted-Ultra fresh parse capability remains unchanged. Scripted parse
requires exactly `Scripted`; Ultra2 still requires its dedicated bootstrap state.
A new read-only retained-Ultra configuration accessor requires the exact new
scope, policy, unused status and zero consumed prefix. This explicit staged
boundary avoids enabling unqualified retained execution merely by constructing
policy-2 owners. Checked planning and scoped retained execution are independently
verifiable subsequent work items.

### Retained Ultra checked plan before scoped execution

The existing `RetainedOptimalPlan` names the Optimal parser family, like
`OptimalParser`; it now carries an explicit copied `PricePolicy` alongside the
resolved strategy. Its old `new`/`for_frame` constructors remain strategy-7-only.
Additive `new_ultra`/`for_ultra_frame` constructors require strategy 8 and bind
policy 2 before sharing schedule, geometry, nine-owner and raw-frame checks.
A private common constructor accepts only the internally supplied policy/strategy
pair. All fields remain private, copied and allocation-free. The old constructors'
validation order and error values are preserved. No new unsafe representation is
needed for this plain plan. Parser and coordinator admission remain separate;
a plan alone cannot choose a provider or change public request resolution.

Every captured policy-2 schedule must reproduce exact owner sizes from its
geometry. The seven parser/search owners match the existing policy-0 geometry;
token owners depend only on the largest block. All prospective public lengths
must use the same balanced schedule and complete raw-output reservation as
retained Optimal. Reject other strategies, insufficient windows, resource
limits, malformed/truncated schedules and sum mismatches before any allocation.
Copied schedules must survive caller mutation. Later coordinator binding checks
policy, source identity, search layout, minimum, target and literal mode before
accepting these bounds. Upstream counterparts remain `ZSTD_compressBlock_opt_generic`
policy 2 and `ZSTD_buildSeqStore`; no upstream code or pointer layout is embedded.

The old frame-plan variant additionally checks its retained policy before raw
output reservation and before cloning the search factory. Wrapping an additive
Ultra plan in the ordinary variant is therefore an early unsupported-feature
error, including when the allocator is configured to fail its next operation.
This check closes the internal dispatch boundary while the dedicated Ultra
frame variant remains unimplemented.

### Explicit retained Ultra execution and coordinator binding

Add `parse_ultra_retained` as a dedicated entry which requires `UltraRetained`
and policy 2, then checks exact source identity and search minimum/target/layout
bounds before mutation. The old `parse_retained` keeps its policy-0 guard. A
private transition helper shares checked block bounds, poisoning, block-local
scratch reset and frequency/search retention only after the selected entry has
validated its scope. It cannot infer admission from source length. Search binding
validation is read-only and reusable during continuation; untouched-start checks
still reject prior queries, Ultra2 search owners or active retained blocks.

`RetainedOptimal` names the parser family and retains a copied policy, with
`from_parts` continuing to admit only the old ordinary configuration. Its new
`from_ultra_parts` requires a checked Ultra plan, the dedicated unused parser,
exact source extent, literal mode, normalized minimum, target and search layout.
It copies the already checked schedule and moves the same seven independent
owners. Every parse dispatches through the corresponding explicitly gated entry.
Pending/committed repeat owners and Ready/Pending/Finished/Poisoned transitions
remain unchanged; reset retains policy and every allocation. The copied tag
shares its safe representation across all storage features. Upstream counterparts
remain `ZSTD_compressBlock_opt_generic` policy 2 and prev/next block publication.

Focused entry tests reject all old constructor scopes before observation, reject
invalid direct block calls transactionally, and release mismatched moved owners.
Exercise all seven independent allocation failures, each publication kind,
observer failures in both blocks, pending/finished reuse and borrowed nonclone
ownership. Complete native state/search/alternative comparisons are a separate
following gate before token and frame integration; public resolution stays closed.


### Retained Ultra complete-state test observers

The test-only comparator borrows an independently validated immutable case and
owns bounded expected/actual path snapshots, a fixed 386-word frequency baseline
and a bounded reconstructed search map. Native observations are expected values,
never parser/provider inputs. Source-only search replay checks every query/map
transition separately, while the coordinator comparison checks every parser,
frequency, alternative, sequence and publication event and each final block map.
Expected sparse baselines persist across blocks; block-local counters reset.

Future-source tests own a changed source vector but mutate only the unconsumed
suffix before borrowing it into the parser. Every completed prefix must retain
all captured states exactly. Later-block alternative-observer failures require
full reset followed by exact complete replay. These are ordinary safe test
records and bounded buffers; the selected production storage features supply
the safe/unsafe representations being compared. No new production owner or
public admission is introduced by these tests.

### Retained Ultra token and shared frame bridge

The private `RetainedOptimalState` gains a distinct `from_ultra_parts` entry;
its old constructor still requires policy 0. The common initializer selects the
matching parser configuration and coordinator constructor only after the entry
has checked its policy. All nine owners retain their separate identities and
exact layout-derived bounds; a short-key owner with no cells still has its own
allocator lifetime. Two token vectors reserve the maximum scheduled block once,
then reuse capacity through parsing, publication, poisoning and reset.

A dedicated private `ReferenceSearchPlan::RetainedUltra` variant is distinct from
`RetainedOptimal`. Both frame reservation and the factory reject a mismatched
variant/policy before allocating output or cloning any child allocator. The
factory moves the dedicated retained-Ultra parser into the same checked token
state and preserves the nine-child clone order. The frame workspace recognizes
both retained variants for exact scheduling, optimal entropy preparation,
pending-repeat validation, raw/RLE rollback, compressed commit and full reset.
Public request resolution remains unchanged until separate qualification.

Tests independently account for exactly nine child allocator identities and all
owner bytes, including the absent short-key allocation, and verify both policy
mismatch directions without allocator contact. Complete captured token sequences
and offset intents, every owned allocation failure, append failure, mixed wire
publication and reset are checked. Actual Rust wire histories require separate
source-derived replay and pinned native decoding; native entropy decisions are
not assumed to match the Rust encoder.

A small foreign-interpreter probe derives the first two 624-byte blocks from the
frozen alternatives_0202 recipe. Independent ordinary/optimized Python proves
all 2752 complete records equal the original prefix after removing the remaining
source. It freezes alternatives 72/103, sequence counts 72/22, tails 0/6 and
candidate repeats [22,164,134]/[61,140,12]. The Rust probe checks these values
across retained publication and reset, without reading observed candidates as
provider input. Its scalar expected records have one safe representation.

### Public retained Ultra routing after private qualification

The public strategy-8 resolver retains the existing fresh path through 16384
bytes. Larger sources enter the separately checked `for_ultra_frame` plan and
`RetainedUltra` dispatch; all resource and complete-output bounds are validated
before allocator cloning or allocation. The source must fit its resolved window,
and 16385..32768 bytes use three or four balanced blocks of at most 8192 bytes.
No new owner representation or unsafe storage is introduced at this routing gate.
The existing nine retained owners and shared entropy/publication contracts apply.

Historical public rejection tests are adjusted only for the intended strategy-8
extension: the new admitted W16 requests reach the injected allocation failure;
explicit W14 counterparts still reject without allocator contact. Strategy9,
all other resource caps and fresh requests retain their previous contracts.
The 1513-case predecessor corpus has exactly one declared response transition;
all other response bytes require direct native-campaign reconciliation.

## Module 22: bounded retained Ultra2 frames (design before implementation)

Pinned upstream `ZSTD_compressBlock_btultra2` (`lib/compress/zstd_opt.c`,
v1.5.7, lines 1517..1543) seeds statistics by scanning only the first block when
five predicates hold: zero literal-length sum, empty sequence store, equal
dictionary and low limits, current position at the dictionary limit, and block
size greater than eight bytes. `ZSTD_initStats_ultra` (lines 1484..1508) also
requires the update frontier to differ from the dictionary limit by at most one
in the upstream unsigned arithmetic. It uses temporary repeats, resets the
sequence store, subtracts that block's size from the logical base, advances
history/update limits and retains learned frequencies. Later blocks take one
ordinary policy-2 pass with retained history.

Module 19 scans the whole source because its fresh scope has one block. Its
`Ultra2Coordinates::rebase` therefore uses the complete source length. Reusing
that calculation unchanged for retained frames would invalidate the wrong
interval. The new plan must use the first block length as its logical bias while
retaining checked access to the whole admitted source prefix. The existing
fresh constructor and coordinate contract remain unchanged.

A dedicated retained-Ultra2 capability and checked plan distinguish strategy 9
from retained strategy 8 despite their shared policy-2 price arithmetic. Old
constructor scopes and parser entries remain closed to the new capability.
The private plan copies explicit two-to-four-block schedules; the public
16385..32768-byte extension uses the same balanced three/four-block schedule as
retained Optimal and Ultra. It validates the complete borrowed source/window,
nine owner bounds, output reservation and first-block coordinate bias before
allocation. Only the first actual block can trigger bootstrap. The discarded
pass must not inspect future blocks or publish tokens, repeats or wire entropy.

After an eligible first-block seed, the normalized logical origin is the first
block length and the complete logical end is that origin plus the frame source
length. For the proposed public bounds this end is at most 40960. A later block
keeps that same origin; it does not rebase again. Source access maps an index by
checked subtraction of the origin and then checks against the visible physical
prefix. Stale first-pass indices below the new origin are never source addresses.
The frame content size and admitted window span remain the physical source
length. A skipped bootstrap retains origin zero. Native transport records must
preserve this bias after removing only the native constant offset of two.

The coordinator owns phase and block progress while retaining the existing seven
parser/search owners and two token scratch owners. It separately tracks initial
repeats, discarded candidate repeats, actual pending repeats and committed wire
state. Bootstrap resets phase-local path/candidate cursors and discarded token
counts, preserves allocated path/search cells and learned frequency statistics,
and invalidates first-pass history through checked integer coordinates. Dormant
path cells are retained exactly as in the existing fresh Ultra2 observer;
resetting logical progress must not silently zero those physical cells. No pointer-base subtraction or unchecked wraparound enters source
access. Raw/RLE publication on any actual block preserves committed repeats and
entropy while retaining actual-pass search/price history. Reset restores the
complete startup state without allocation and permits the same bootstrap once
again. Failure after mutation poisons the coordinator until reset; invalid
plan/source/phase combinations reject before mutation or allocation.

Performance-sensitive owners reuse their independently selectable safe/unsafe
backends. Copied schedules, phases, coordinates and repeat records share one
plain-value representation. The no_std plus alloc boundary remains unchanged.
Native observations, independent source-only models and borrowed Rust fixture
readers cover both passes of the first block, the rebase, every later block,
publication kinds, future-source isolation and failures in each phase. Private
first-block sizes of eight and nine distinguish threshold behavior independently
of public admission; every public first block is above the threshold. Full
foreign Miri and concrete no-atomics probes precede public admission.

The current 1593-request public corpus has two prospective predecessor
transitions: index 1512, `retained-resource-ultra2`, and index 1592,
`retained-ultra-resource-ultra2`. Both use 32768 bytes and resolve to
W15/C12/H12/S6/minimum 3/target 4095/strategy 9; their request levels are 11 and 13.
An eventual public extension must declare these exact changes before execution
and preserve the other 1591 predecessor responses. W14 requests exceeding their
window remain resource rejections. Dictionary/prefix input, window expiry,
numeric streaming, long-distance matching, workers and complete upstream
migration remain separate scope.


The source model must distinguish physical frame offsets, logical search indices
and the normalized Rust index space. Event `ip`, `anchor`, query positions and
sequence anchors remain physical offsets into the complete source. Search-table
cells and both insertion frontiers retain their logical bias, including across
sparse snapshot baselines. Only source access translates the bias. Catch-up at
later block entry compares `origin + block_start` with the retained frontier;
using the physical start would silently skip the strict 384/192 transition.
The visible source view is restricted to the current block's consumed prefix,
including during the discarded first pass; later source bytes never become a
search input early. First-block threshold controls include seven, eight and nine
bytes: upstream's sequence-store entry skips only sizes below seven.

The independent model's compatibility obligations are distinct. A one-block
reconciliation path must reproduce Module 19's existing seeded and skipped
observations without changing its public packet grammar. A bootstrap-disabled
retained path must reproduce Module 21's policy-2 state and publication records.
The new transport carries explicit algorithm phase and block identity; neither
is inferred from coincidental table contents or reused padding-pass counters.
A new native observer must record the actual wrapper's predicate decision,
first-pass reset and subsequent block transitions without replacing upstream
parser/provider statements. Predictions alone do not establish native parity.

### Independent retained Ultra2 fixture reader

The additive `ZU2RFX01` validation pack borrows each case's ASCII name, source,
complete frame and encoded byte range from one immutable input slice. It owns
only bounded, fallibly reserved observation vectors. No fixture observation is
used as a production search candidate or codec implementation.

The case header records source/parameter bounds, two-to-four actual blocks,
complete frame extent, path-cell count, bootstrap scope and policy. Separate
native/sanitized allocator ledgers precede the borrowed byte ranges. Each block
records its physical extent, actual publication kind, pending/committed repeat
owners, entry frontier, final candidate repeats, tail and logical sequence count.
Its eligibility record precedes one or two phase envelopes. Each phase contains
its origin, caller repeats, complete sparse parser/search/frequency deltas,
append-only sequences, final frequency/candidate state and optional discarded
pass reset. Physical path and search baselines persist across every phase and
block; frequency baselines change only through recorded rescaling/refresh.

The reader checks every count against both an explicit cap and remaining bytes
before allocation. Indices and source ranges use checked arithmetic. It derives
bootstrap eligibility, first-block rebase, later insertion catch-up, frequency
rescaling, repeat publication, owner swaps and complete frame extents independently
of the producer. Query footprints are limited to the consumed physical prefix;
logical search indices retain the first-block bias and stale indices never become
source addresses. Raw/RLE publication retains committed repeats; only compressed
publication swaps the committed owner. The discarded pass cannot publish tokens.

The projection preserves exact native inputs and all four equal frame outputs.
It validates complete native records before converting them and rejects missing,
duplicate or extra records. The independent reader's negative cases cover
truncation, oversized counts, malformed deltas, eligibility/rebase changes,
discarded-state leakage, later-block coordinates, coherent frequency changes,
repeat-owner publication and frame payload extents. Plain copied observation
records use one representation; this test-only safe reader needs no artificial
unsafe backend.

### Constructor-only retained Ultra2 capability

`OptimalParser::from_ultra2_retained_parts` consumes the existing frequency,
path and candidate owners without cloning or allocating. It accepts only policy
2 and a physical source length of 14..32768, leaving complete schedule/window
validation to the subsequent checked plan. Initialization retains the existing
minimum-match/target bounds and resets all three owners through their shared
safe/feature-selected contracts. Invalid construction drops each consumed owner
exactly once. Reset reuses those allocations.

The new plain `Scope::Ultra2Retained` discriminant is distinct from both fresh
Ultra2 and retained Ultra. Its configuration accessor accepts only unused,
ready, uninitialized-frequency state. Every existing parser/provider/rebase
entry rejects it before mutation. This first capability step deliberately has
no execution entry; later phase execution must explicitly require this scope.
Its upstream counterpart is the workspace ownership spanning first-block
`ZSTD_compressBlock_btultra2` bootstrap and later policy-2 block calls, rather
than an upstream standalone constructor. The discriminant and borrowed source
record need no independent unsafe representation.

### Checked retained Ultra2 schedule and source coordinates

`RetainedUltra2Plan` is an allocation-free copied value. It owns at most four
block lengths and validated parameters, and records the maximum block length,
all nine existing owner byte bounds and the complete known-size frame bound.
Private schedules contain two-to-four blocks of 7..8192 bytes, sum exactly to
14..32768 physical source bytes and fit the resolved window. Public-plan-only
construction balances 16385..32768 bytes into three/four blocks. Neither factory
changes resolver or encoder admission.

The plan uses `Ultra2Coordinates::new(first_block_length, source_length)` only
when the first block exceeds eight bytes; otherwise origin remains zero. It
never calls the fresh plan's whole-source rebase. Visible coordinates accept
only complete scheduled prefixes, keep the same logical bias for every later
block and reject stale/future addresses by checked subtraction and extent.
The plan validates all owner `Layout` bounds and the full frame header plus
source and per-block headers before allocation. Existing fresh Ultra2 and
retained Optimal/Ultra validators retain their original bounds; an additive
layout method handles the tiny retained threshold controls. Upstream counterparts
are first-block `ZSTD_initStats_ultra` rebase, `ZSTD_buildSeqStore` scheduling and
known-size frame/header reservation. Copied fields share one representation
across all independently selected storage backends.

### Plan-bound retained Ultra2 search phases

`OptimalSearch::from_ultra2_retained_parts` moves the existing head, tree-node,
short-key and candidate owners into a dedicated capability holding a copied
validated plan and a rebased flag. The source remains the same immutable borrow.
Old constructors keep this capability absent; all old query and block-entry
paths reject it before accessing source or tables. Search start validation
requires the exact source identity, plan geometry and untouched frontiers.

A new source-view setter installs checked physical prefix length and logical
origin without changing the underlying borrow. First-block seeding is mandatory
exactly when the plan requires it. Rebase requires the first seeded block,
retains every physical table cell, resets both frontiers to the first-block bias
and clears only candidates. Actual encoding must then enter the first block
before any later contiguous scheduled block. The paired parser establishes pass
completion; the search alone does not claim that parsing has finished.

Block entry applies upstream's strict 384/192 frontier catch-up in logical
coordinates before copying the frontier for short keys. Query input positions
remain physical offsets, with checked lookahead and repeats; only the dedicated
query boundary adds the logical origin. Existing source readers reject stale
indices before access and cap every read at the current block's physical end.
Reset clears tables/frontiers, restores origin zero and reuses all allocations.
This composition reuses each owner's independent safe/unsafe backend; the copied
plan, rebased flag and ranges need no additional unsafe representation.

### Scoped retained Ultra2 parser and phase coordinator

`RetainedUltra2` owns seven independently allocator-parameterized parser/search
workspaces, a copied checked plan, current block position, two repeat arrays and
pending actual-pass result. Its lifecycle is Ready, Pending, Finished or
Poisoned. It never allocates or stores tokens. Construction requires the dedicated
unused parser and search scopes, exact borrowed source and matching plan, policy,
literal mode, minimum match and target. Previous constructors cannot enter it.

The dedicated parser entry validates source, schedule, repeats and phase before
mutation. A discarded seed leaves the actual retained cursor at zero. Only its
successful completion permits rebase; the rebase preserves frequency/path/search
owners while resetting phase-local cursors, candidate storage and initial repeats.
Actual encoding advances the retained cursor. Later blocks retain that logical
bias and learned frequency state. The parser's existing path algorithm is shared;
its provider is explicitly restricted to the new search capability.

The coordinator observes upstream eligibility, phase begin/end and discarded reset
through the existing `Ultra2Observer` contract. A small borrowed observer adapter
counts sequence literals without allocating or collecting discarded tokens. Only
an actual encoding result becomes pending publication. Raw/RLE publication keeps
committed repeats; compressed publication swaps the two wire owners. Every
post-entry error poisons the coordinator, including observer failures before or
after any phase; whole-owner reset reuses allocations and permits bootstrap once
again. Upstream counterparts are `ZSTD_compressBlock_btultra2`,
`ZSTD_initStats_ultra`, `ZSTD_buildSeqStore` and prevCBlock/nextCBlock publication.
The new coordinator and phase values use one safe representation; existing owners
retain their independent opt-in unsafe implementations. Token/frame integration
and public strategy admission remain separate work.

### Actual-pass tokens and private retained Ultra2 frame dispatch

`RetainedUltra2State` owns the seven-owner coordinator and the same two token
vectors used by earlier retained strategies. It reserves their maximum-block
bounds before phase execution. A borrowed observer forwards only actual encoding
events to the collector; every discarded seed event is ignored by token scratch,
while the coordinator independently observes and counts the learning pass.
The new tiny-block collector accepts 7..8192-byte blocks and initial repeat eight
for a seven-byte prefix. The previous retained collector keeps its 64-byte lower
bound. Both use checked source-prefix ranges and the same offset resolution.

Only a successful actual pass exposes tokens. Scheduling/repeat errors precede
reservation; later failures hide partial tokens until complete reset. Publication
validates the expected committed repeats before changing shared output/entropy.
Raw/RLE kinds retain committed history and compressed kinds replace it. Reset
preserves all nine allocator identities and capacities.

A private `ReferenceSearchPlan::RetainedUltra2` variant binds the existing shared
frame encoder to this state. The resolver does not produce this variant yet.
Its copied plan supplies exact schedule and full frame reservation, and the
factory clones exactly once for each of nine owners in the established order.
The shared optimal entropy path and transactional publication hooks are reused;
no new wire coding or unsafe representation is introduced. The initial frame
checks cover Rust decoding, complete actual wire histories, raw/RLE rollback,
allocation and output-append failures, and reset. Independent replay under actual
wire kinds, native decoding, platform gates and public admission remain separate
qualification steps.

## Retained Ultra2 public admission candidate

The public resolver keeps the fresh Ultra2 plan for known sources through16384
bytes and delegates larger sources to the already checked RetainedUltra2Plan.
That plan admits16385..32768 bytes in one window, bounds W/H/C/S/target before
any owner is cloned, and derives a balanced three/four-block schedule. It owns
only value records and indices. The existing nine independent parser/search/token
owners, shared entropy workspace and output allocator retain their contracts;
no new allocation, unsafe representation or C binding is introduced by admission.

The first scheduled block alone performs discarded learning, checked rebase and
actual encoding. Every later block uses one retained Ultra-priced pass. Public
source hints and immutable requests still flow through known-size resolution.
The two declared predecessor resource-to-success changes are independently
frozen at indices1512/1592; every other prior complete response must remain equal.
Original fresh and sibling-family states remain independently bounded. Public
admission requires focused Rust, exact predecessor/native interoperability and
independent controls; full regression/platform/benchmark closure follows.

## Caller-owned one-shot decompression output (Module 23)

`SliceHistory<'a>` owns one exclusive borrow of an initialized destination slice,
not its allocation. Its private position, frame start, window and effective
limit satisfy `frame_start <= position <= limit <= output.len()`. Construction
clamps the requested output limit to destination capacity. Starting a frame
moves frame start to position, excluding all earlier concatenated-frame bytes.
Every append/repeat checks the remaining allowance before any write. Match
validation rejects zero, cross-frame and out-of-window distances before capacity
checks, matching `History`; even a zero-length match requires a valid offset.
Forward sequential expansion handles overlap, including bytes produced by the
same match. Index arithmetic is bounded by the initialized slice and the checked
end. No output allocation, reserve, ownership conversion or scratch history copy
is permitted. This corresponds to prefix/destination bounds in pinned v1.5.7
`ZSTD_decompressFrame` and `ZSTD_decompressBlock_internal`.

The default backend uses safe slice operations. Independent Cargo feature
`unsafe-slice-history` selects the same checked match wrapper with raw pointer
reads/writes. The live exclusive slice borrow proves allocation lifetime and
aliasing; checked distance/end prove every source/destination is in bounds;
all bytes are initialized and u8 alignment is one. No set_len or uninitialized
storage is involved. Plain indices/options remain shared value representations
and need no artificial unsafe operations.

Private `FrameOutput: SequenceOutput` adds `start_frame`, `frame_bytes` and
`repeat`. A generic one-shot frame loop borrows a sink and scratch allocator;
`History<A>` keeps ownership of its original `Vec<u8,A>`. Preserve the old
wrapper's early empty-input rejection, scratch clone order/lifetime and return
of that vector. The helper clones scratch only at the original SequenceState and
literal-allocation points. Header limits, raw-payload truncation before append,
content-size checks and checksums retain their original error ordering.
This shares the orchestration represented by upstream `ZSTD_decompressMultiFrame`
and `ZSTD_decompressFrame` without introducing dynamic dispatch or duplicated
frame parsing. Entropy/literal scratch owners remain independent of destination.

Public names are `decompress_into`, `decompress_into_in`,
`decompress_into_with_options` and `decompress_into_with_options_in`. Arguments
are input, mutable output, optional DecodeOptions, optional allocator, in that
order; result is `Result<usize>`. Default output allowance is output.len(); the
options variant uses min(output.len(), max_output_size), leaving max_window_size
independent. The allocator owns only scratch and may be a reference to a
nonclone allocator. Success returns a valid prefix and preserves its suffix.
Errors may leave provisional bytes inside the effective allowance, but must
preserve bytes beyond it. No whole-frame transactional rollback is promised.

Empty encoded input remains UnexpectedEof; actual empty frames and skippable-only
streams can succeed with empty output. The dictionary-free frame coverage and
existing dictionary-ID rejection are preserved. Dictionaries, in-place aliasing
and stable incremental output sessions are separate scopes. Rust shared input
and exclusive output borrows rule out overlapping input/output in safe callers.
Native zero-capacity and null-destination cases are distinct oracle checks;
Rust's API accepts an empty slice rather than a nullable pointer. The focused
scope, corpus adapter and verification work items are recorded in TODO.md and
`caller-output-design.md` before implementation.

## Stable borrowed incremental output (Module 24 design)

This design is recorded before implementation. Qualification and publication
follow Module 23 closure; its pinned validation source remains unchanged while
an independent candidate is developed. Full contracts and upstream mappings
are in `stable-output-design.md`.

`SliceDecoder<'output, A = Global>` owns one exclusive output borrow through
`SliceHistory<'output>`, an allocator, frame/phase records, inline header scratch,
a bounded encoded-block vector and existing Huffman/sequence owners. It never
retains an input borrow. It has no ring history or decoded pending-output vector.
`SliceHistory` remains the sole output-position owner, with unchanged
`frame_start <= position <= limit <= output.len()` invariants. New private
read/reset operations expose only written bytes or reset indices without
mutating destination bytes. The checked `unsafe-slice-history` backend is reused;
state enums, counters and borrowed-slice descriptors are shared safe value
records, requiring no artificial unsafe implementation.

The phase enum covers header, block header, raw, RLE-byte, RLE, compressed,
skippable length/payload, checksum, ended and failed. Header scratch is `[u8;18]`
with a bounded filled count. Frame metadata records a validated `FrameHeader`,
maximum block size, last-block bit and remaining payload. All address-bearing
state consists of slice borrows, indices or owned vectors. A compressed-input
vector never exceeds the validated encoded block size; complete payloads already
present in a call can decode directly without this copy. Frame-local Huffman and FSE overrides are released
between frames and on reset; the sequence allocator remains owned, and encoded
input scratch capacity may be reused. All requests are fallible
and belong to fresh clones of the selected scratch allocator. The caller's
destination is never resized, adopted or deallocated by the codec.

Raw input appends incrementally, RLE expands directly after receiving its byte,
and complete compressed payloads execute existing literal/sequence decoders
against `SliceHistory`. Frame-local output supplies backward references and,
after the final block, checksum validation. Frame completion excludes old
history before the next standard frame. Skip payloads are consumed without
output. Checked input/completed-frame counters and the history's written length
report progress; failure poisons the context until reset. Format errors may
leave provisional output within the allowance, never outside it.

Construction clamps output allowance to destination capacity. `process(input)`
returns `Progress` with `NeedInput` or `FrameFinished`; insufficient fixed output
is `ResourceLimit`, not a request to replace the borrowed destination. `finish`
seals a clean boundary after at least one complete frame, while partial/empty
streams fail. `output()` lends the current prefix through `&self` and rejects
failed sessions. Output in an unfinished frame remains provisional. `reset`
retains the same borrow/options and reusable workspace, resets position to zero
and leaves bytes unchanged. Rust borrowing excludes output mutation, replacement
and conflicting views while a session or an output view is used.

Pinned upstream counterparts are `ZSTD_DCtx`, `ZSTD_checkOutBuffer`,
`ZSTD_decompressContinueStream` and `ZSTD_decompressStream` with
`ZSTD_d_stableOutBuffer`. Pointer/capacity/position consistency becomes a Rust
lifetime and encapsulation property. The upstream `zstreamtest.c:786-863`
assertions map to complete and fragmented output, short destination rejection,
and borrow compile-fail checks. Native workspace sizes and C numeric return
hints are not Rust ABI promises. Dictionaries, prefixes, stable input,
overlapping in-place decoding and configurable external workspaces remain
separate scopes. The existing buffered dictionary-capable `Decoder` is unchanged.

## Dictionary-backed caller-owned output (Module 25 design)

Design and decomposition precede candidate implementation; promotion follows
Module 24 closure. `dictionary-output-design.md` records exact API, error and
qualification scope. Public `decompress_into_with_dictionary` and `_in` borrow
a destination and a persistent shared/view dictionary only for the call.
Scratch uses `A: Allocator + Clone`; dictionary storage can use another allocator
and remains independently borrowed. No destination ownership is adopted.

`DictionarySliceOutput<'output,'dictionary>` owns a checked `SliceHistory` value
and a copied `DictionaryView`. It is an allocation-free reference/index record,
with no artificial unsafe backend. `FrameOutput` and `SequenceOutput` delegate
ordinary operations while dictionary matches use one checked slice operation.
The slice keeps `frame_start <= position <= limit <= output.len()`. Immutable
dictionary bytes, content start, ID and prepared entropy retain their existing
validated owner/view invariants and lifetime. Prepared tables are borrowed
fallback; fresh frame state resets repeat offsets to dictionary values and
replaces working tables independently.

At current-frame position p, distance d<=p is a frame match and requires
0<d<=window. A dictionary match d>p requires p<=window and k=d-p no larger than
dictionary content. It begins at content.len()-k, copies up to k dictionary
bytes and then frame positions starting at zero, including earlier writes
within the same forward match. Validate the complete destination end and initial
source ranges before writing. Initial dictionary distance may exceed window;
do not reapply the ordinary frame-only window test when this same match crosses
into frame output. This preserves the existing documented p>window rejection.

`unsafe-slice-history` selects the checked destination/read loop, while
`unsafe-dictionary` independently selects validated immutable dictionary reads.
Default implementations use safe indexing. Every raw-pointer operation must
prove initialized bounds, u8 alignment, nonzero forward distance, exclusive
output borrow, shared dictionary lifetime and lack of aliasing. No pointer is
stored beyond its call. Extreme distances/lengths and all failures validate
before any match write; zero-length calls still validate offsets.

The generic one-shot frame loop takes an optional immutable dictionary view,
checks IDs, seeds repeat offsets and calls existing dictionary-view entropy
entrypoints. Old wrappers pass None, preserving clone order, error ordering
and allocation behavior. Upstream counterparts are `ZSTD_decompress_usingDDict`,
`ZSTD_decompressMultiFrame`, `ZSTD_decompressFrame` and dictionary sequence
execution; the existing Rust buffered `BlockSink` supplies the established
window/expiry contract. Prefix and stable-session dictionary support are
separate future scopes.

## Persistent dictionaries in stable output (Module 26 design)

Record this design before implementation; qualify and promote after Module 25
closes. `stable-dictionary-output-design.md` defines the API and evidence scope.
`SliceDecoder<'output,A=Global,D:DictionaryHandle=SharedDictionary>` owns an
optional persistent attachment alongside its existing scratch and borrowed
`SliceHistory`. A shared handle retains the immutable dictionary; a view keeps
an explicit owner borrow. Never store a view into the session's own attachment.
Normal constructors preserve None and their existing allocation/clone behavior.
New `with_dictionary`/`with_dictionary_in` constructors consume the supplied
attachment, keeping its allocator independent from scratch allocator A.

A temporary dictionary sequence sink borrows `&mut SliceHistory` and a view
computed from the attachment only during a block call. It delegates append/len
and checked dictionary match expansion. This is a plain safe reference record,
not a new unsafe storage family. Existing independent dictionary/slice backends
retain all window, source range, capacity, initialization, alias and overlap
proofs. The optional attachment uses SharedDictionary's existing Arc/Rc selection;
auto traits follow D and A. Input fragment borrows are never retained.

At each standard header, validate nonzero IDs against the selection and seed
repeat offsets from prepared entropy or defaults. ID zero still uses selected
content. Prepared table views are temporary fallback; owned overrides are
frame-local. Skips do not consume the attachment. Completion/reset releases
Huffman/FSE overrides, retains sequence allocator and encoded-input capacity,
and restores clean history boundaries. Reset retains dictionary selection,
output borrow and effective limits; no attachment replacement API is admitted.

Keep `retained_capacity()` scratch-only, excluding both output and dictionary.
New `dictionary_retained_capacity()` reports reachable dictionary payload
capacity, zero when absent, without claiming unique ownership or including
shared-control/allocator-internal allocations. Lifetime tests cover output
views, destination mutation, dictionary view escape and shared-owner drop.
Native stable-output DDictionaries, actual guarded destinations, exact/short
capacities and the documented p>window difference require independent checks.
A fixed seven-byte persistent dictionary corpus replay is a labelled supplement,
not an original one-shot fuzz target's streaming branch. Prefixes, workspace
replacement and dictionary-parameter mutation remain separate modules.

## Single-use prefixes in stable output (Module 27 design)

`stable-prefix-design.md` records this design before implementation and limits
promotion to after Module 26 closure. Replace the optional stable-session
attachment with private `Attachment<D>`: None, Persistent(D), PendingPrefix(D)
or ActivePrefix(D). It owns at most one handle/view and never stores a view into
itself. Claim moves PendingPrefix to ActivePrefix after successful standard
header parsing, before ID/resource checks. Skips and incomplete/rejected header
parsing leave it pending. Completion/reset drops active prefixes; reset preserves
persistent or unclaimed selection. Failure retains active ownership until
reset/drop. Persistent frames do not introduce handle clones.

Boundary-only set_dictionary/set_prefix replace selection transactionally while
preserving fixed output, counters and scratch. A clean Header phase with no
buffered bytes admits changes; every other phase rejects without mutating the
session. Constructor helpers admit shared owners or borrowed views independently
of scratch allocator A. Reachable dictionary capacity counts the single
populated variant, separate from scratch. The enum is a plain safe value record;
existing independent dictionary/slice storage features handle actual byte
access. Upstream counterparts are ZSTD_DCtx_refPrefix_advanced, ZSTD_getDDict's
use-once transition and referenced DDict selection. Rust claim/reset and
configuration boundaries are explicitly bounded, not opaque C context parity.

## Dictionary-free in-place output (Module 28 design)

`in-place-output-design.md` records this prospective module before implementation;
promote it only after Module 27 closes. One `InPlaceBuffer<'a>` exclusively borrows
the complete shared source/output allocation. Input is a suffix selected by an
index; output starts at zero. Positions and ranges replace independently aliased
Rust input/output references. Validate every cursor addition, source range,
`frame_start <= output_position <= output_limit <= buffer.len()` and unread-input
fence before access. A framing-only `FrameLayout` record owns no allocation or
borrowed payload and remains an ordinary safe value in every backend.

For compressed blocks, split at the validated payload start: output/history is
the mutable left half, encoded bytes the immutable right half. An ephemeral
`SliceHistory` resumes validated frame/output indices under that temporary limit;
all borrows end before advancing input and reborrowing the full allocation. Raw
blocks use checked overlapping movement within the unique slice, bounded by the
consumed raw payload end so future input is untouched. RLE copies its byte into
a value and uses a conservative payload-start write fence. Frame boundaries
reset history eligibility; checksum validation reads only current-frame output.
No full input copy, output Vec or cloned history is introduced.

A dedicated `unsafe-in-place-buffer` feature is planned for the overlapping raw
move, equivalent to safe `copy_within`. It must prove initialized source/dest
bounds, alignment, unique owner lifetime and permitted overlap before ptr::copy;
no pointer is stored. Compressed matches retain independent slice-history and
entropy implementations. Plain coordinates/layout records need no unsafe variant.
Existing fallible allocator-owned literal/entropy scratch remains separate from
the caller allocation; borrowed nonclone allocators follow established APIs.

The allocation-free margin scan sums actual frame headers, three bytes per block,
optional checksums and complete skippable sizes, then adds the largest advertised
block maximum. Checked arithmetic rejects overflow. It traverses framing without
claiming payload/checksum validity and does not replace runtime write fences.
Upstream counterparts are ZSTD_findFrameSizeInfo, ZSTD_decompressionMargin,
ZSTD_decompressFrame's oBlockEnd protection and ZSTD_copyRawBlock's memmove.
The original single/two-frame in-place fuzzer assertions, actual overlapping
native guards, malformed layouts, allocator failures, foreign interpretation,
no_std and identical safe/unsafe measurements are separate required gates.

### Module 28 checked storage implementation

`SliceHistory::resume` validates `frame_start <= position <= limit <= slice.len()`
without clamping invalid state. `InPlaceBuffer` has private visibility, no retained
pointers and no allocations. Successful raw/RLE/publication transitions preserve
`output_position <= input_position`; all rejection branches precede mutation.
Its new independent feature changes only checked raw overlapping movement.
The integrated storage tests include real split-borrow history resumption.

## Persistent dictionaries in one-shot in-place output (Module 29 design)

This analysis precedes implementation; `dictionary-in-place-design.md` records
scope and qualification. Predecessor production and measurement snapshots stay
frozen, and no dictionary in-place API is promoted before Module 28 closes.

`InPlaceBuffer` keeps its single unique source/output slice and validated indices.
The selected `DictionaryView` is a separate immutable borrow, with no cloned handle
or allocator and no copied dictionary/history payload. Rust's borrow checker must
reject a view into any region of the exclusively borrowed in-place buffer.
`DictionarySliceOutput::resume` composes checked `SliceHistory` coordinates with
that view; it is a plain wrapper requiring no artificial unsafe representation.
Existing dictionary, slice-history and raw-movement features remain independent.

The frame coordinator receives an optional view and checks nonzero IDs before
frame setup. Every standard frame restarts repeat offsets from prepared fallback
and resets frame-local output eligibility; skippable frames keep the selection.
Huffman/FSE fallback stays borrowed and immutable, while overrides and literal
scratch belong to independently cloned scratch allocators. Compressed payloads
remain disjoint from resumed writable history until checked publication.
Dictionary-to-frame matches preserve existing window-expiry rules and never read
previous-frame output as history. The reference counterparts are pinned v1.5.7
`ZSTD_decompress_usingDDict`, `ZSTD_decompressMultiFrame`,
`ZSTD_decompressBegin_usingDDict` and dictionary-aware sequence execution.

Public dictionary wrappers borrow shared/prepared/raw views, accept independent
scratch allocator types, and preserve all suffix-input/failure bounds. The test
plan separately qualifies exact native shared-buffer layouts, allocator failure
ordinals, frame resets and borrowed lifetime/alias boundaries. Single-use prefixes,
incremental overlap and external workspaces remain outside this module.

### Module 30 frame metadata and structural size queries

Before implementation, `docs/frame-scan-design.md` records the shared checked
frame traversal, copied header/kind/count values and borrowed skippable payload.
The private frame layout retains metadata already parsed for the margin scan;
public query records own no bytes or allocations. Every extent is validated by
the existing cursor before constructing values or borrowing a payload. Safe and
unsafe configurations share these plain records; the existing cursor feature
controls memory access, with no artificial metadata-only unsafe operations.

Concatenated advertised sizes and decompression bounds accumulate in checked u64
arithmetic, including bounds above 4 GiB on 32-bit hosts. Unknown size is an Option,
not a reserved integer. Full traversal rejects trailing malformed ranges even
after unknown sizes, unlike native early-return shortcuts. Structural success
does not validate block content, checksum truth or actual decoded length.
The pinned counterparts are ZSTD_findFrameSizeInfo, ZSTD_findFrameCompressedSize,
ZSTD_findDecompressedSize, ZSTD_decompressBound and skippable frame queries.
Native sentinel/window-cap/wrapping behavior remains explicitly distinct.

## Module 31: scalar compression bounds and borrowed skippable serialization

`compression_bound` owns only scalar sizes. Native counterparts are v1.5.7
ZSTD_COMPRESSBOUND/ZSTD_MAX_INPUT_SIZE and ZSTD_compressBound. Width-specific
admission and checked arithmetic return a typed result; there are no sentinel
values, allocations or retained references. The same value representation is
shared across feature configurations, with no artificial unsafe operations.

`write_skippable_frame` temporarily borrows one exclusive output slice and one
independent immutable payload. Variant, representable u32 length, usize header
addition and actual capacity are validated before any mutation. Standard slice
copies serialize fixed little-endian header arrays and payload; errors leave
all output unchanged and successes preserve the unused suffix. No alias or
reference is retained. The checked length helper admits numeric-limit tests
without constructing invalid huge slices. This transient serialization adds no
owned storage structure or allocator; keep it core-only and forbid unsafe code.
Native counterpart: ZSTD_writeSkippableFrame. Error precedence is explicit Rust
policy, not native numeric-code equivalence. See `frame-utilities-design.md`.

## Module 32: one-shot selection owner and transient call claim

`OneShotDecoder<D>` owns copied DecodeOptions and exactly one private selection:
none, persistent D, or next-call D. D implements the sealed DictionaryHandle and
may be a shared owner or a borrowed immutable view. The existing sealed handle
contract includes Clone; this coordinator never invokes it. Private selection
and claim records impose no Clone bound of their own.
Replacement drops the previous owner. No compressed input, output or scratch
allocator is retained. Existing one-shot engines own all per-call frame/scratch
state. The coordinator is not a reusable native scratch workspace.

A transient call record borrows persistent D or owns a removed next-call D.
The latter is removed before any attempted decode and lives through all frames
in that call, then drops on success/error/unwind. A failed call cannot reinstall
it. These plain ownership enums have shared safe/unsafe representations; their
work delegates to separately selectable storage backends without raw operations.
Native counterparts are ZSTD_getDDict, ZSTD_decompressDCtx and
ZSTD_DCtx_refPrefix_advanced. Unlike incremental prefixes, next-call selection is
consumed even by skippable-only/empty attempts. See `one-shot-selection-design.md`.

## Module 33: bounded frame sink and one-block encoded staging

`FrameSink<'a>` owns one exclusive initialized caller output slice and a position
bounded by its length. Checked append validates addition/capacity before mutation,
copies an independent immutable source, then advances position. A failed append
preserves that append's bytes and position; prior successful appends remain.
The performance-sensitive copy has safe and opt-in unsafe-frame-sink paths with
identical runtime bounds and alias/lifetime contracts. No buffer allocation is
owned, and scalar position needs no artificial unsafe operation.

A fallible allocator-owned Vec stages one encoded block at a time, including its
header, then is cleared/reused. Header/checksum records are small initialized
values. Existing BlockEncoder owns independent match, entropy and ring scratch;
there is no new full-frame output Vec or full-input copy. Each allocator clone
owns/frees its own storage. The call owns all scratch and returns only the number
of bytes in caller storage. Counterparts are the destination capacity/position
and block emission in ZSTD_compressCCtx/ZSTD_compress2; output bytes follow the
existing Rust Greedy one-shot algorithm, not the native parser. See
`slice-encode-design.md` for provisional-output and excluded dictionary/reference/
streaming contracts.

## Module 34: borrowed dictionary selection for bounded frame output

An optional DictionaryView supplies immutable content/entropy/ID ranges to the
existing BlockEncoder while FrameSink and a fallible allocator-owned Vec emit one
block at a time. The view owns no allocation and cannot outlive its dictionary.
Its plain metadata is shared across safe/unsafe configurations; no handle clone
or artificial unsafe operation is necessary. Existing search, history, entropy
and bounded-copy owners keep their independent features. Header ID serialization
uses the existing initialized record. No input, output or dictionary borrow is
retained after the call. Dictionary allocation remains independent of scratch;
each scratch clone frees its own domain. Counterparts are the bounded output and
dictionary attachment of ZSTD_compress_usingDict/ZSTD_compress_usingCDict. Exact
Rust output follows compress_with_dictionary, not the native parser. See
`dictionary-slice-encode-design.md` for capacity and ownership qualification.

## Call-scoped dictionary selection for compression (Module 35 design)

OneShotEncoder<D> owns only the existing Selection<D> record. Persistent owners
are borrowed; next-call prefix owners move into Claimed until an attempted call
returns or unwinds. The stored selection clears before output admission or
allocator work. DictionaryView copies borrow immutable content and metadata;
no handle clone, copied dictionary history or retained input/output/scratch is
introduced. Existing sealed DictionaryHandle includes Clone, but the coordinator
does not invoke it. Plain enums share their safe representation across features.
The underlying bounded emitter retains every existing safe/unsafe storage choice.

Upstream counterparts are pinned ZSTD_CCtx_refCDict, ZSTD_CCtx_refPrefix_advanced,
ZSTD_compress2 and ZSTD_CCtx_init_compressStream2. Native lazy dictionary setup,
workspace/parameter/session lifetimes and error precedence are not implied.
Contracts and evidence requirements: `one-shot-encode-selection-design.md`.

## Parameterized dictionary-backed one-shot compression (Module 36 design)

A combined attempt owns no new state: a resolved parameter record (plain copied
window/hash/chain/search logs, min_match, target_length and reference
strategy) and an immutable DictionaryView enter each caller-owned call exactly
like the stateless dictionary view does today. Dictionary preparation stays
preconstructed; only indices seed the selected bounded profile's storage under
the existing per-call seed contract. Frame flags follow the existing
parameterized-profile family (known content size, no checksum) plus the
dictionary ID field the dictionary paths already emit; modules 33–35 keep
their exact ordinary checksummed bytes. The resolved window expires dictionary
content as ZSTD_checkDictValidity does, within each profile's existing window
and table caps. No options record, workspace or live context is retained
between calls; coordinator selection semantics are unchanged.

Upstream counterparts are pinned ZSTD_compress_advanced_internal and
ZSTD_compress2 combining ZSTD_getCParamsFromCCtxParams resolution with a
referenced CDict, plus dictionary loading in ZSTD_compressBegin_internal.
Native CCtx parameter lifecycle, reset directives, LDM, workers and arbitrary
frame settings are not implied. Contracts and evidence requirements:
`one-shot-encode-parameters-design.md`.
