# Allocator preparation

Module 9 is now decomposed in `TODO.md`. The complete ownership/API design is
in [allocator-design.md](allocator-design.md), with original test contracts in
[upstream-allocation-plan.md](upstream-allocation-plan.md). The Rust 1.100
installation attempt returned HTTP 404 on 2026-10-09; the installed 1.101
nightly separately compiled a no_std standard-API probe without feature gates.
The implementation compiler is now pinned to `nightly-2026-10-08`, reporting
Rust 1.101.0-nightly at commit `1d81eb4ad9cd207e3e638bd32b17ec4fce8412a6`.
The package's conservative version floor is `1.101`; only the exact dated
compiler is supported by the current verification. Stable 1.100 and stable
1.101 are not claimed as tested. Module 8 retains its Rust 1.89 evidence.

Custom allocation is being implemented incrementally. Existing public codec
APIs still use Global until their individual checkpoints are complete. Dictionary
construction now exposes `from_bytes_in`, `from_bytes_with_mode_in`, and
`raw_in`; its payload and prepared tables use the chosen allocator. Standard
shared handles still allocate the counts and inline dictionary object in Global.
`Dictionary::view()` provides an allocation-free borrowed alternative; codec
context integration follows its own checkpoint. The
chosen standard `Allocator`/`Vec<T, A>` subset compiles without feature gates
on the pin. Custom Arc/Rc, allocator-aware BinaryHeap, and fallible Box
construction still require unstable `allocator_ext` in the inspected library;
the design uses checked typed storage and an explicit dictionary-sharing
contract instead. No substitute allocator trait is exposed.

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

## Implementation and verification

`TODO.md` tracks the active per-owner migration. Default convenience APIs retain
Global results. Custom constructors create each owner with its own allocator
value, including independent clones; moving or swapping storage moves that
allocator too. A borrowed `&A` permits a non-Clone allocator and avoids cloning
user allocator state. Ordinary allocator cloning may panic or allocate outside
the selected domain; only codec-owned collection requests are covered by the
fallible allocation guarantee.

The separate test allocator checks every selected positive-size request,
failed-growth preservation, distinct clone identities, ownership transfer,
reset, and final release. It does not weaken the production safe-default lint.
Verification must include Global/custom correctness, every unsafe feature,
no_std, Miri, independent official-codec interoperability, and identical-input
allocation/time measurements. Fully controlled dictionary allocation needs a
caller-owned prepared view: convenient Global Arc/Rc sharing allocates both
reference counts and the inline dictionary object outside the custom domain.

The exact scope of upstream custom-memory reporting, static workspaces,
estimators, and shrinking tests is recorded separately; ordinary Rust allocator
parameterization does not implement all those contracts.
