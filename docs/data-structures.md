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

These are planned designs. Future feature names below are reservations, not
currently supported Cargo features.

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
