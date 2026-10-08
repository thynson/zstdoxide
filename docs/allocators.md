# Allocator preparation

The initial crate targets Rust 1.89.0 and uses the global allocator through
`alloc::vec::Vec`. Custom allocators are not implemented yet. This keeps the first
codec version usable on the tested installed toolchain while leaving a concrete
migration path to the allocator API requested for Rust 1.100.

The installed Rust 1.101.0-nightly `rust-src` was inspected on 2026-10-08.
`library/core/src/alloc/mod.rs` marks `Allocator` stable since 1.100.0, and
`library/alloc/src/vec/mod.rs` gives the same stabilization version for `Vec<T, A>`
and `Vec::new_in`. The allocator work should target those APIs after adding an
actual Rust 1.100 build to the test matrix. This is source inspection evidence,
not a claim that the current crate has been built with Rust 1.100.

## Existing boundaries

| Owner | Current allocation sites | Planned allocator-aware storage |
| --- | --- | --- |
| Decoded output/history | `History::reserve` | `Vec<u8, A>` retained by the output/history owner |
| FSE decoding tables | `FseTable::from_normalized` | `Vec<FseEntry, A>` with the allocator carried by the table |
| Huffman tables and literals | `HuffmanTable::read`, literals and stream decoding | Typed tables and literal buffers parameterized by the same allocator type |
| Match candidates | `MatchTable::new` | An owned initialized position array allocated in `A` |
| Compression workspace | Greedy literals/sequences and inverse entropy tables | Reusable typed `Vec<T, A>` buffers |
| Compressed output/bit writer | `compress`, `BitWriter::write` | Output and staging vectors allocated in `A` |

All growth triggered by input already uses `try_reserve` or `try_reserve_exact`
with checked capacity arithmetic where needed. The byte/bit readers borrow
slices; frame metadata and XXH64 lanes allocate nothing. Indices survive buffer
movement. Unsafe paths obtain addresses only after reservation and keep no raw
pointer across a possible reallocation. These properties are independent of
which allocator backs a vector.

## Work decomposition for a later module

1. Add Rust 1.100 CI and verify the stable allocator API and supported targets.
2. Choose context/output APIs that carry an allocator type without introducing
   self-referential borrows; preserve current convenience wrappers using `Global`.
3. Parameterize output/history first, then entropy tables and compression
   workspace. Each allocation remains paired with the allocator stored by its
   owning collection; never free through an unrelated allocator instance.
4. Add a counting/failing allocator to exercise every fallible allocation and
   prove allocation failure leaves owned structures valid. Every allocator
   `unsafe impl` and raw allocation operation needs its own complete proof.
5. Run the existing safe/unsafe, oracle, Miri, and performance matrices for both
   global and custom allocation paths. Measure allocation counts alongside time.

Public allocator-aware signatures remain a later design decision. The codec
does not emulate an allocator trait or expose a temporary incompatible trait.
