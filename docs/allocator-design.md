# Module 9: allocator ownership and platform validation

Design recorded on 2026-10-09 before Module 9 implementation. The executable
checkpoint list belongs to `TODO.md`. Module 8 validation uses its frozen
Rust 1.89 source snapshot. The compiler checkpoint below precedes generic codec
migration and does not rewrite that historical evidence.

## Scope and implementation direction

Parameterize the existing typed owners with the standard Rust allocator API.
Retain one codec algorithm and the existing safe/unsafe access backends. Do not
create a substitute allocator trait, duplicate the codec for older compilers,
reinterpret an unaligned byte arena as typed storage, or introduce a production
C allocation adapter.

Default convenience APIs should continue to use the global allocator. Custom
allocation is an explicit caller choice through allocator-aware constructors and
functions. The core remains `no_std` plus `alloc`; selecting an allocator does
not itself require unsafe codec code. A caller's implementation of the standard
unsafe `Allocator` trait carries its own obligations.

Complete allocation control includes temporary tables, serialization buffers,
priority queues, and dictionary ownership. Redirecting only the final output
vector is a useful intermediate checkpoint, not completion of this module.

## Verified toolchain evidence and decision boundary

Before the Module 9 compiler checkpoint the repository pinned Rust 1.89.0 and
declared `rust-version = "1.89"`. Tools observed during the initial review were:

| Toolchain | Compiler evidence | Installed targets observed |
| --- | --- | --- |
| Module 8 pin | 1.89.0, commit 29483883e, 2025-08-04 | aarch64-apple-darwin |
| stable | 1.99.0, commit b940084d7, 2026-09-28 | aarch64-apple-darwin; aarch64-unknown-linux-gnu |
| nightly | 1.101.0-nightly, commit 1d81eb4ad, 2026-10-07 | aarch64-apple-darwin |

The coordinating agent attempted:

```sh
rustup toolchain install 1.100.0 --profile minimal --component rust-src --component rustfmt --component clippy
```

That attempt returned HTTP 404 for the official Rust 1.100.0
`aarch64-apple-darwin` distribution checksum. Rust 1.100 was not installed.
This is a concrete missing compiler verification, not evidence that the APIs
are absent from the installed nightly, nor a claim about every distribution
host or future release.

The coordinating agent also compiled `/tmp/zstdoxide-allocator-api-probe.rs`
successfully with the actual installed 1.101 nightly compiler. That source uses
`#![no_std]`, `#![forbid(unsafe_code)]`, no `#![feature]`, the standard `Allocator`,
`Vec<u8, &A>::new_in`, `Vec<u8, Global>::new_in`, and fallible reservation. Its log
is `/tmp/zstdoxide-allocator-api-probe-nightly.log` (empty successful output).
This is actual 1.101 compiler evidence; it must not be described as a 1.100 run.

Installed nightly library-source inspection establishes these boundaries:

| API | Annotation observed |
| --- | --- |
| `Allocator`, `AllocError`, `Global` | Stable since 1.100.0 |
| `Vec<T, A>`, runtime `Vec::new_in`, allocator access | Stable since 1.100.0 |
| `Vec::new_in` evaluated in a const context | Still unstable |
| `Box<T, A>` allocator parameter | Stable since 1.100.0 |
| `Box::try_new_in` | Unstable `allocator_ext` |
| `Arc<T, A>` and `Rc<T, A>` allocator parameters and constructors | Unstable `allocator_ext` |
| `BinaryHeap<T, A>` and `BinaryHeap::new_in` | Unstable `allocator_ext` |
| `AllocatorClone` and `StaticAllocator` | Unstable `allocator_ext` |

Source anchors under the installed nightly sysroot's
`lib/rustlib/src/rust/library` include `core/src/alloc/mod.rs:196`,
`alloc/src/vec/mod.rs:439` and `:1063`, `alloc/src/boxed.rs:473`,
`alloc/src/rcs/arc.rs:265`, `:700`, and `:923`, `alloc/src/rcs/rc.rs:325`,
`:684`, and `:861`, and `alloc/src/collections/binary_heap/mod.rs:274` and `:556`.
These are local source observations, not a guarantee that every annotated
feature behaves identically in an unavailable earlier compiler.

The selected implementation compiler is `nightly-2026-10-08`, successfully
installed with rustfmt, Clippy, rust-src, and Miri on 2026-10-09. Its exact
compiler commit is `1d81eb4ad9cd207e3e638bd32b17ec4fce8412a6`, reporting
`1.101.0-nightly`, LLVM 23.1.3. The repository pin, CI jobs, Miri guard, and
benchmark default use that dated toolchain. The package declares
`rust-version = "1.101"` as a conservative version floor. The supported compiler
is the exact pin; this is not a claim of testing a future stable 1.101 release
or the unavailable 1.100 distribution. No unstable feature gates or
`RUSTC_BOOTSTRAP` are needed for the chosen standard API subset.

Before changing codec storage, run the unchanged production source under this
compiler with default/all-feature tests, safe/all-backend no_std compilation,
strict Clippy, and formatting. Record any compatibility corrections explicitly.
The Module 8 records retain their original Rust 1.89 compiler and source hashes.
A later available Rust 1.100 compiler must be tested explicitly before lowering
or claiming that minimum; do not create duplicate codec representations solely
to maintain the old compiler pin.

## Production allocation map

Every row must be covered before claiming complete custom allocation. Types
that contain other owners inherit those owners' allocator requirements.

| Owner or path | Current storage and temporary allocations | Migration boundary |
| --- | --- | --- |
| `History` | Whole output `Vec<u8>` | Retains and returns its own allocator-bearing vector; frame indices remain relative to initialized bytes |
| `RingHistory` | Reusable window `Vec<u8>` | Reserve before reset; retain high-water capacity; absolute positions remain `u64` |
| `BitWriter` | Reusable encoded `Vec<u8>` | Reservation before unsafe writes; clear retains allocation |
| `entropy_encode` | Count-description writer bytes, normalized-count vector, remainder-ranking vector, weight-stream output and bit-writer storage | Thread allocator through every intermediate construction, not just the returned description |
| `FseTable` | Initialized decoding-entry vector; `decode_weights` output vector | Inline RLE state stays allocation-free; normalized construction and scratch weights use the selected allocator |
| `FseEncodingTable` | Dense transition vector and per-symbol initial-state vector; temporary decoding table when built from normalized counts | Both vectors and the intermediate decoder table belong to their selected allocator; no partial inverse table is published |
| `HuffmanTable` | Weight vector, decoding-entry vector, decoded literal/output vectors, four-stream combined output | Reader, raw/RLE literal branches, and every stream-output helper must accept the allocator |
| `HuffmanCodebook` | Inline initialized 256-entry code array | No allocator parameter needed solely for the value record |
| `BuiltLiteralTable` | Inline codebook plus owned serialized-description vector; temporary validation decoding table | Preserve description/code pairing and allocator ownership through validation and transfer |
| `huffman_encode` | Full/sparse description vectors, FSE weight output, and `BinaryHeap<Reverse<(weight, symbol, index)>>` in `tree_lengths` | The production tree heap is a second allocator limitation, in addition to the training heap |
| `LiteralEncoder` | Reusable streams vector, complete-section vector, bit writer, selected table-description temporaries | Candidate failure does not publish a tree; accepted bytes and codebook remain paired |
| `fse_select` | Owned candidate descriptions and optional replacement inverse tables; normalization temporaries | Borrowed predefined/repeat choices stay borrowed; owned candidates carry their allocator through commit/discard |
| `SequenceEncoder` | Three default tables, three current slots, three spare slots, planned sequences, three symbol vectors, serialized section, bit writer | Swap entire owners on commit; no allocator-detached allocation enters a spare slot |
| `MatchTable` | Initialized position vector | Seed and working tables are independent allocations; copied indices borrow no seed storage |
| `MatchChain` | Initialized heads and cyclic nodes vectors | Slot/tag invariants and bounded node counts are unchanged; seed-copy operations allocate only during construction |
| `BlockEncoder` | Literal, logical-sequence, and payload vectors; match/chain seeds; retained ring; literal/sequence subcontexts; shared dictionary handle | Allocator reaches all lazy construction paths, including strategy-specific storage and dictionary setup |
| `Encoder` | Input staging block, pending encoded bytes, block workspace, persistent/prefix/active dictionary handles | No whole-frame buffering; reset retains typed storage and preserves existing dictionary/prefix rules |
| `Decoder` | Encoded block staging, pending decoded bytes, ring history, current entropy tables, persistent/prefix/active dictionary handles | Advertised-window and output-budget checks precede reservation; provisional output and error poisoning remain documented |
| One-shot compression | Returned encoded vector plus `BlockEncoder` graph | Construct the result in its final allocator type from the start |
| One-shot decompression | `History` result, or a streaming decoder plus collected result for dictionaries | Do not accidentally leave dictionary-based one-shot scratch on Global while the plain path is custom |
| `Dictionary` | Serialized byte vector and optional Huffman/FSE decoding tables | All payload/table storage has an allocator; immutable content ranges never borrow another field |
| `SharedDictionary` | Global `Arc<Dictionary>` on pointer-atomic targets; global `Rc<Dictionary>` otherwise | Current fixed-size ownership allocation is infallible and explicit; stable custom Arc/Rc is not available in the inspected API |
| Raw training | Sample-window vector, 65,536 frequency cells, candidate `BinaryHeap`, chosen-range vector, final content vector | Samples remain borrowed; selected bytes and all working storage are owned by the selected allocator |
| Formatted training/finalization | Raw selection graph when used, literal serializer graph, three normalized-count descriptions, assembled dictionary bytes, validation tables and copied published bytes | Include every transient allocation; any future owned-byte parser must preserve validation before publication |

`ByteCursor`, reverse-bit readers, frame/block metadata, sequence plans and
scalar states, repeat triples, fixed histograms/tree arrays, and XXH64 have no
owned heap allocation. Do not add allocator parameters or artificial unsafe
operations to those plain values unless a containing API needs the type for
another owned field.

The test-only `EntropyOutput` and `BuiltStandaloneTable` also own vectors and
compose the same writers/table builders. They should follow generic defaults
when allocator tests use them, but are not extra production allocator promises.
Filesystem/process/oracle allocations remain test or adapter concerns outside
the `no_std` core guarantee.

## API shape and zero-copy Global convenience results

After the compiler decision, prefer a single idiomatic generic implementation:
`Encoder<A = Global>`, `Decoder<A = Global>`, and `Dictionary<A = Global>`.
Existing convenience constructors remain available on the Global specialization.
Allocator-aware aggregate constructors accept `A: Allocator + Clone` by value;
leaf owners require only `A: Allocator` when they need no allocator clone.
Exact dictionary-view type parameters should be settled at the separate
dictionary checkpoint rather than leaked through every earlier API.

Possible public shapes are:

```text
impl Encoder<Global> { new() -> Self }
impl<A: Allocator + Clone> Encoder<A> { new_in(allocator: A) -> Self }
compress(input) -> Result<Vec<u8, Global>>
compress_in<A: Allocator + Clone>(input, allocator: A) -> Result<Vec<u8, A>>
```

These signatures illustrate allocation ownership; constructors that currently
perform fallible work retain their Result contract. The default wrapper calls
the same generic implementation with `Global`. Each aggregate creates its
children using clones of its allocator, and each child stores the exact value
it used to allocate its own blocks. The returned vector is constructed with
its final allocator type at entry. This preserves a zero-copy Global result
without an output conversion, an extra lifetime parameter, or two codec paths.

Ordinary `Clone` need not produce equivalent allocators. This design does not
require equivalence: a newly constructed vector allocates and deallocates through
its own retained clone, and no existing block is freed through another clone.
The installed nightly's `Vec<T, A>::Clone` uses precisely this ownership shape:
`self.allocator().clone()` followed by a fresh `to_vec_in` allocation
(`alloc/src/vec/mod.rs:3901`). That library method is still unsuitable for codec
buffer copies because it is infallible; reserve fallibly and push initialized
`Copy` elements into proven spare capacity instead. The general Vec contract
permits bulk insertion to reallocate despite spare capacity, so a durable
fallibility guarantee must not rely solely on the current implementation of
`extend_from_slice` or `resize`. Whole vectors or complete table owners may be moved or swapped,
including current/spare entropy slots, because their allocators move with them.
Never separate a block from its allocator, infer interchangeability from equal
allocator types, or introduce a raw-parts conversion to resolve a type mismatch.

Non-Clone allocator values remain usable through a borrowed reference. For an
external `&'a A`, the standard blanket implementation supplies `Allocator`, and
the reference itself implements `Clone` without cloning `A`. Thus the same APIs
naturally accept `Encoder<&'a A>` and return `Vec<u8, &'a A>`. Callers must keep
that allocator alive until every owner is dropped. Contexts contain external
references, never references into one of their own fields. This also gives
allocation tests a simple way to route all requests through one observed
allocator instance. If an owned allocator's Clone deliberately chooses separate
resources, accounting must follow those actual resources rather than assume a
single allocation domain.

Ordinary `A::clone()` may unwind or allocate independently of `A`'s allocation
methods. Only the stronger, unstable `AllocatorClone` trait adds a no-unwind
clone requirement. Public allocation guarantees cover codec-owned collection
requests, not arbitrary user allocator cloning behavior: a clone panic cannot
be converted into `Error::AllocationFailed`, and a clone's private allocation
need not use the selected domain. Passing `&A` avoids cloning the allocator
value itself.

An independently generic workspace/output driver is an optional extension for
an actual caller-owned-output API, not a prerequisite for Global convenience:

```text
run_compression<WorkspaceAllocator, OutputAllocator>(
    context: &mut Encoder<WorkspaceAllocator>,
    input: &[u8],
    output: &mut Vec<u8, OutputAllocator>,
) -> Result<()>
```

Such a public API should specify whether it appends or clears. Streaming output
already uses caller-owned slices, so do not add another public surface solely
to hide an extra copy. Do not borrow an allocator from inside the output vector
and then mutate that vector while workspace storage retains the borrow.

Arc/Rc cloning uses the stronger unstable `AllocatorClone` bound in the inspected
source. That is a different operation: shared handles must keep the same owned
allocation valid and therefore need compatible allocator ownership. Do not
propagate that unstable bound into the Vec-based codec solely because dictionary
sharing will need a separate contract.

`Vec::new_in` is runtime-stable only. Generic constructors should be ordinary
functions. Keep existing const Global constructors only where useful and
supported, rather than adding unstable const features.

## Dictionary sharing and priority queues

The stable subset does not currently support custom `Arc`/`Rc` control blocks.
Choose and document one of these staged contracts:

- Keep Global `Arc`/`Rc` ownership around allocator-aware dictionary payloads.
  Report that ownership allocation as an explicit exception. It contains the
  reference counts and the entire inline `Dictionary<A>`, not just a header;
  dictionary vectors and table buffers remain separately allocator-owned.
- Add borrowed prepared-dictionary views for contexts whose caller requires all
  allocation to use the provided allocator. The caller owns the dictionary and
  its allocator; the borrow prevents either from disappearing while in use.

A dictionary view should expose immutable bytes, content ranges, repeat values,
and validated table views. It should permit workspace and dictionary storage
to come from different allocator instances, without copying a prepared dictionary
merely to match the context's allocator type. Preserve existing persistent and
one-use-prefix behavior. Do not hand-roll reference counting to conceal a gap
in the currently stable standard API.

Both `huffman_encode::tree_lengths` and raw training use `BinaryHeap`. Stable
complete custom allocation therefore needs a checked heap backed by `Vec<T, A>`
or a separately justified allocation-free bounded representation for the
256-symbol Huffman case. Preserve the exact comparator/tie order and index
invariants. Tree weights, serialized descriptions, dictionary selection, and
frame fingerprints must remain unchanged after this mechanical storage change.
If a new unchecked heap backend is later justified, give that distinct storage
family its own feature and safety/benchmark checkpoint. Do not silently attach
it to the frequency-cell backend.

## Lifetime, failure, and publication contracts

1. A collection owns both initialized elements and the allocator value/reference
   responsible for its allocation. Move, swap, and return the whole owner.
   Cloned allocator values may be different allocation domains; a fresh owner
   allocates through its own clone and never adopts a block from another value.
2. Allocator and dictionary lifetimes cover every context and result that uses
   them. No context is self-referential, and no public mutable allocator access
   can invalidate storage while elements remain live.
3. Indices, lengths, ranges, and generation tags remain the authority for access.
   Allocation capacity is not initialized length. Existing per-family bounds
   checks continue to guard unsafe paths in release builds.
4. Reserve before taking pointers used by an unsafe operation. No saved pointer
   survives an operation that may grow, shrink, replace, or free its collection.
5. Constructors drop all partially built owners on failure and publish no
   incomplete table, codebook/description pair, or dictionary.
6. A failed sequence/literal candidate leaves committed entropy and repeat
   offsets intact. Discard releases or retains candidate storage according to
   the existing bounded-slot contract; reset must not publish stale state.
7. `RingHistory::start_frame` reserves before clearing the previous frame.
   Failure preserves its existing valid storage and state.
8. Streaming process errors preserve the existing poisoned-until-reset contract.
   Do not promise a new transactional rollback of already accepted input or
   provisional output. Reset must restore a usable context while preserving
   the documented allocation and dictionary/prefix lifetimes.
9. Returned allocation errors map to `Error::AllocationFailed` consistently.
   Explicit size/budget validation remains `ResourceLimit` or the existing
   parameter error. A caller-provided allocator may itself abort under its
   standard trait contract; the codec cannot transform that into a Result.
   Ordinary allocator cloning may separately panic or allocate, as described
   above; element copies inside fallible buffer helpers use `Copy` values.
10. Allocation accounting distinguishes requested layouts, collection capacity,
    allocator rounding, live/peak bytes, and any retained Global ownership
    handles. Existing `retained_capacity` is not silently relabeled total heap
    consumption.

The standard allocator contract also forbids unwinding from allocation,
deallocation, growth, shrinkage, and allocator destruction. Test allocator
implementations need complete safety contracts covering identity/equivalence,
lifetime, alignment, initialization, zero-sized requests, failure preservation,
non-overlap and matching deallocation. Existing access backends can rely on the
same typed-Vec invariants whether its allocator is Global or custom.

## Small implementation checkpoints after the decision

Each item is independently reviewable and committed only after focused
verification. The coordinator records executable scope in TODO before any
implementation; this design alone marks no implementation as complete.

1. Record the actual compiler decision, allocator API subset, and matrix policy.
2. Finalize the allocation map and ownership/error guarantees; identify any
   deliberately retained Global allocation explicitly.
3. Parameterize `History`, preserving a zero-copy Global result and independent
   custom-output construction; verify overlap, limits and failure preservation.
   Include an allocator whose clones have distinct identities to prove that
   moving/swapping owners never changes which instance frees a block.
4. Parameterize `RingHistory`; verify wrap, reset, window growth, and allocation
   retention without changing frame positions or backend proofs.
5. Parameterize forward/count bit writers; verify identical emitted bits and
   untouched state on reservation failure.
6. Parameterize FSE decoding storage and weight scratch; verify all saved tables,
   malformed inputs and dictionary singleton behavior.
7. Parameterize inverse FSE storage and selection temporaries; verify exact costs,
   support holes, ownership transfer, and every existing unsafe-access boundary.
8. Parameterize Huffman decoding/literal buffers and description construction;
   preserve inline-versus-dictionary limits and paired codebooks.
9. Replace the Huffman builder's Global-only heap using a checked representation;
   compare all tree/code/description oracle vectors before further integration.
10. Parameterize literal and sequence contexts; preserve default/current/spare
    slots, commit/discard behavior and reusable capacities.
11. Parameterize decoder orchestration and collecting one-shot paths; verify
    fragmented raw/RLE/compressed frames, dictionaries, checksums and errors.
12. Parameterize match tables/chains, seeds and block/stream encoder storage;
    verify all strategies, output fragmentation, resets and byte fingerprints.
13. Add allocator-aware dictionary payload preparation and the chosen sharing or
    borrowed-view contract, including independent allocator lifetimes.
14. Replace the training heap and parameterize windows/frequencies/selection;
    preserve selected bytes and bounded sample behavior exactly.
15. Parameterize formatted finalization and its validation allocations; preserve
    dictionary bytes, IDs, table descriptions and independent C acceptance.
16. Add counting/failing allocator integration tests, exercising each allocation
    attempt, failed growth, reset after failure, drop and ownership transfer.
17. Run the complete Global/custom × safe/individual-unsafe/all-unsafe matrix,
    including explicit C interoperability and focused Miri allocator proofs.
18. Add platform checks and execute supported targets separately from compilation.
19. Benchmark identical fingerprints with allocation counts/peak bytes as well as
    time, then record limitations and close only the completed module scope.

The default codec should continue to forbid unsafe code when no unsafe access
backend is enabled. Put unsafe test allocator implementations in a separate
integration-test crate or external harness, rather than weakening the core's
safe-default lint. Per-instance failure injection is preferable to globally
failing allocations made by the test harness or unrelated threads.

## Platform evidence plan

Current local execution evidence is Apple/AArch64. The repository's CI describes
Linux jobs, but a workflow file is not evidence that its current revision ran.
The installed Linux target is associated with Rust 1.99, not the pinned 1.89.
No emulator or additional target was installed for this review.

Use separate small checkpoints for:

- Native Linux/x86-64 execution on the selected compiler, including saved codec
  fingerprints and standard/custom allocation tests.
- A 32-bit target: compile first, then run with an actual supported runner or
  interpreter. Check `u64` frame counters, `usize` conversion rejection,
  addressable windows, capacity arithmetic and dictionary offsets.
- Big-endian execution or supported Miri interpretation for byte readers,
  entropy bitstreams, checksums and frame output. Do not infer it from little-
  endian success or from compilation alone.
- A target without pointer-width atomics, such as a suitable Cortex-M profile,
  to compile the `Rc` dictionary path with `no_std + alloc`. Runtime execution
  additionally needs a suitable harness and allocator; compilation is only the
  first claim.
- Every unsafe feature independently on the applicable selected target, followed
  by all compatible features together. Preserve actual test counts in Miri
  logs so an obsolete selector cannot pass by running zero tests.

Resource-heavy original corpora and performance matrices should remain separate
from quick allocation-boundary and platform checks. Compare performance within
one host/configuration; do not aggregate different architectures into a single
speedup claim.

## Completion criteria and open decisions

A complete custom-allocation claim requires coverage of the production map,
including temporary heaps and dictionary ownership, or an explicitly narrower
published API contract. Global control-block exceptions must remain visible.
The complete upstream allocator/workspace test migration is a separate claim:
this design does not implement C callback allocators, static workspaces,
in-place compression, numeric levels, or every upstream custom-memory API.

The compiler policy is the dated 1.101 nightly above. Dictionary migration will
preserve the convenient Global shared handle and separately add a caller-owned
prepared view for fully controlled allocation, each with its own checkpoint.
The current 1.100 HTTP 404 remains a recorded verification gap; the successful
1.101 no-feature compiler probe does not test the unavailable compiler.

## Aggregate integration refinement

Before implementing dictionary/context integration, use immutable slice-backed
FSE/Huffman views to erase only allocator type from borrowed prepared tables.
No owner allocation changes allocator, and no table copy is needed merely to
select frame-owned versus dictionary-owned decoding tables. Dictionary views
contain private validated fields and are tied to the owner's lifetime.

The public streaming integration may use a defaulted dictionary-handle type
parameter alongside allocator `A`, preserving existing type spellings. A small
sealed statically dispatched handle contract can support Global Arc/Rc handles
of `Dictionary<B>` and caller-owned `DictionaryView<'d>` independently of `A`.
Exact constructors and exports belong to the integration checkpoint. Persistent,
prefix and active fields must store complete handles; temporary views must
never be stored as references into an adjacent field. Prefix claim, skippable
frames, and reset semantics remain unchanged. Switching handle representations
may require a new context; clearing a borrowed handle does not promise to
shorten the context type's compile-time lifetime.

The decoder integration is divided into whole-buffer output, incremental
context, and dictionary-based collection checkpoints. Whole-buffer output
retains the original allocator value in `History<A>` and uses fresh allocator
clones for per-frame tables/literal scratch. The streaming type will be
`Decoder<A = Global, D = SharedDictionary>` with the sealed `DictionaryHandle`
bound, allocator-aware `new_in(options, allocator)` on the default handle
specialization, and `with_dictionary_in(options, dictionary, allocator)` where
both types are inferred from arguments. Existing Global constructors stay
available. Persistent, prefix and active fields store `D`; block sinks borrow
only validated dictionary views and typed history/output owners. Reset retains
the allocator factory; a changed window limit replaces the complete ring owner
using a fresh clone. Every runtime allocation failure still poisons the context
until reset, including failures after provisional output or prefix claim.
