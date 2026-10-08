# Architecture and delivery plan

## Goal and compatibility boundary

The project reimplements Zstandard in Rust. It does not use the C library or
another Rust codec to perform compression or decompression. The official
[v1.5.7 implementation](https://github.com/facebook/zstd/tree/v1.5.7) is a pinned
study reference and independent test oracle. The
[Zstandard format specification](https://github.com/facebook/zstd/blob/v1.5.7/doc/zstd_compression_format.md)
is the interoperability contract.

The first codec milestone targets one-shot, non-dictionary frame decoding,
including raw, RLE, and compressed blocks, FSE sequences, Huffman literals, table
reuse, and content checksums. Its encoder must produce interoperable frames,
beginning with raw/RLE blocks and then a basic match strategy. Supporting the
format does not require matching the reference compressor's output bytes or
compression ratio. Each unsupported feature must return a specific error rather
than silently changing semantics.

That milestone is not full upstream parity. Streaming, prepared dictionaries,
dictionary training, advanced parsing/compression levels, long-distance matching,
multithreading, legacy formats, the C ABI, and the complete upstream CLI each need
their own later module and acceptance criteria. Current support belongs in the
README and progress log; this document describes the intended architecture.

## Component boundaries

1. **Memory access:** checked byte readers/writers, forward header bit access, and
   reverse entropy bit streams. Input slices and output ranges are explicit.
2. **Entropy:** FSE distribution/table construction and symbol states; Huffman
   weights, codebooks, and one/four-stream literals. Typed entries replace
   type-punned C allocations.
3. **Block model:** literals, sequences, repeat offsets, block headers, and history
   execution. Entropy decoding produces validated numeric operations before
   copying output.
4. **Frame model:** header parsing, content/window limits, dictionary identity,
   frame checksum, and block iteration. Format metadata is separate from owned
   scratch memory.
5. **Compression:** match finding, sequence selection, entropy encoding, and block
   selection. Raw/RLE fallback keeps valid output available when compression is
   not beneficial.
6. **Adapters:** allocation convenience APIs, reusable contexts, streaming, and
   `std` integration. Filesystem, processes, timing, and CLI operations stay here
   or in test/benchmark tooling.

The [data structure inventory](data-structures.md) records ownership, upstream
counterparts, invariants, and each feature boundary before implementation.
Rust names and `Result`/enum-based APIs are preferred over C naming, sentinel
error encodings, and pointer-to-output parameters. Function documentation may
name corresponding C functions without copying their shape.

## Safe defaults and backend selection

Cargo features independently select `unsafe-byte-cursor`, `unsafe-bitstream`,
`unsafe-history`, `unsafe-fse`, `unsafe-huffman`, and `unsafe-match-table` as those
families are implemented. No unsafe feature is enabled by default. A backend is
private and presents the same safe interface as its safe counterpart. Composite
contexts reuse selected backends rather than duplicating the complete codec.

Checks establishing format validity and resource limits remain active in both
modes. Unsafe code can avoid redundant access checks after validation; it cannot
assume that a caller supplies valid compressed input. Small scalar records use a
shared representation, with the rationale recorded in the inventory. Specialized
pointer-based data structures are added only with a separate comparison and a
complete safety argument.

## `no_std`, ownership, and allocator preparation

The core targets `no_std` plus `alloc`; this permits allocation and does not mean
an allocation-free codec. Borrowed-slice parsing and caller-provided output APIs
can operate without owning the input. `Box`, `Vec`, and optional `Arc` express
ownership; indices express locations. Shared dictionary APIs must account for
targets without pointer-width atomics.

Allocation should be concentrated in output growth, tables, history, dictionary
preparation, and reusable workspaces. Prefer `try_reserve` and explicit error
propagation where storage grows from input. Check content/window/output limits
before allocation and use checked size arithmetic. A declared content size is
untrusted metadata, not permission to allocate arbitrary memory.

Future allocator parameterization can introduce context/workspace constructors
that accept a storage provider or allocator, while leaving slice-based algorithms
unchanged. Do not bake global allocation assumptions into table lookup or match
execution. Allocations must be released with the allocator that created them;
resizing must preserve initialized regions and invalidate saved addresses.

The installed nightly standard-library source marks `Allocator`, `Vec<T, A>`,
and `Vec::new_in` stable since Rust 1.100. The initial version uses Rust 1.89.0
without unstable features. [The allocator plan](allocators.md) records that
source evidence and the required migration and validation work. Explicit
allocation boundaries are preparation, not a claim that custom allocation
already works or that a Rust 1.100 build has been tested.

## Incremental implementation order

| Module | Work decomposition before implementation | Acceptance boundary |
| --- | --- | --- |
| Foundation | Project instructions; source/data structure analysis; crate and feature boundaries | English documentation, safe default build, and core `no_std` compilation |
| Bit and entropy primitives | Byte access; reverse bit stream; FSE distributions/tables; Huffman weights/tables; independently gated backends | Known examples, exact end handling, malformed-input cases, safe/unsafe agreement |
| Frame decoder | Frame/block headers; output/history and repeat offsets; literal modes; sequence execution; XXH64; frame orchestration | Official-codec differential decoding, checksums, limits, truncation, and multi-block table reuse |
| Initial encoder | Valid frame writer; raw/RLE selection; indexed match table; sequence/entropy writing; block decisions | The official decoder accepts every generated frame; round trips cover incompressible and repetitive data |
| Validation and measurement | Oracle harness; deterministic corpus; feature matrix; primitive/codec benchmarks | Checked-in reproducible commands and actual recorded results, including slower backends and unavailable checks |
| Streaming and dictionaries | Resumable phases; bounded ring history; raw dictionary/prefix; prepared dictionary tables; dictionary IDs | Arbitrary input/output chunking and reference interoperability with dictionary fixtures |
| Advanced compression | Lazy/optimal parsing; match chains/trees; long-distance matching; block splitting; tuning | Compression ratio and resource cost characterized against the pinned reference |
| Allocators and broader parity | Storage/workspace provider; supported allocator API integration; optional threading and remaining adapters | Explicit compatibility matrix and independent checks for each added capability |

The coordinator decomposes the active module into small checkbox items in
`TODO.md` before code changes. Each completed item is checked and committed with
its focused validation. At a module boundary, remove the completed checklist,
record the outcome in `docs/progress.md`, and retain a decomposition for the next
module. Git history preserves the detailed record. A failed or unverified item
stays open.

## Validation strategy

Use three distinct evidence layers: focused format cases, independent official
interoperability, and generated malformed-input/fuzz cases. A Rust encoder/decoder
round trip alone can preserve a shared bug. The official codec is test-only and
must never be linked into the library's implementation.

Compare default, each unsafe feature alone, and all unsafe features. Test release
mode so debug-only checks cannot hide invalid assumptions. Exercise short and
unaligned inputs, limits, large logical lengths, entropy-table reuse, zero-literal
repeat offsets, and overlapping matches. Add 32-bit target checks and Miri when
available; report unavailable infrastructure explicitly. Require formatting,
Clippy, focused tests, and a `no_std` build at module milestones.

Correctness comes before throughput. Benchmark primitive access and whole-codec
work separately, on the same deterministic inputs and compiler settings. Record
compressed size alongside encode/decode speed so a different compression choice
cannot be mistaken for an unsafe access improvement.
