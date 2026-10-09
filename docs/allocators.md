# Allocator ownership and validation

Module 9 has connected every codec allocation owner to the public allocator APIs.
Final integrated validation and measurement are tracked in `TODO.md`. The ownership/API design is
in [allocator-design.md](allocator-design.md), with original test contracts in
[upstream-allocation-plan.md](upstream-allocation-plan.md). The Rust 1.100
installation attempt returned HTTP 404 on 2026-10-09; the installed 1.101
nightly separately compiled a no_std standard-API probe without feature gates.
The implementation compiler is now pinned to `nightly-2026-10-08`, reporting
Rust 1.101.0-nightly at commit `1d81eb4ad9cd207e3e638bd32b17ec4fce8412a6`.
The package's conservative version floor is `1.101`; only the exact dated
compiler is supported by the current verification. Stable 1.100 and stable
1.101 are not claimed as tested. Module 8 retains its Rust 1.89 evidence.

Custom allocation is available across one-shot compression/decompression,
streaming Encoder/Decoder contexts, raw/formatted dictionary construction and
raw/formatted training. Convenience APIs retain Global defaults; allocator-last
`_in` entry points use the chosen allocator family for codec payloads and
scratch. Returned output and serialized dictionary bytes retain the original
allocator, with fresh retained clones for separate temporary/table owners.
Dictionary and context allocation types can differ through `DictionaryHandle`.
`Dictionary::view()` provides an allocation-free borrowed attachment; standard
`into_shared()` uses an infallible Global allocation for both reference counts
and the entire inline dictionary object. Full integrated checks and allocation
benchmarks remain in the Module 9 checklist. The
chosen standard `Allocator`/`Vec<T, A>` subset compiles without feature gates
on the pin. Custom Arc/Rc, allocator-aware BinaryHeap, and fallible Box
construction still require unstable `allocator_ext` in the inspected library;
the design uses checked typed storage and an explicit dictionary-sharing
contract instead. No substitute allocator trait is exposed.

See [allocator usage](allocator-usage.md) for runnable examples and compile-time
dictionary lifetime checks.

## Existing boundaries

| Owner | Allocation sites | Allocator-aware storage |
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

The per-owner migration records are in [allocator-validation.md](allocator-validation.md).
Default convenience APIs retain
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

## Compiler, safety, and remaining contracts

The final production ownership review found no open routing, fallibility,
allocator-transfer or unsafe-access issue in the 29 recorded source files.
[The review record](evidence/2026-10-09-allocator-ownership-audit.json) identifies
its exact hashes and scope. [Failure coverage](allocator-failure-validation.md)
links the executed ordinal sweeps and reset/drop assertions. These are evidence
for the selected implementations and workloads, not a proof of every possible
caller allocator or input.

[Focused platform validation](allocator-platform-validation.md) records 38
selected earlier Miri executions plus 12 final public-API follow-ups across
32-bit little-endian and 64-bit big-endian targets. The no-pointer-atomics ARM
caller verifies `Rc` selection and generic API compilation. Miri interprets
selected programs; ARM validation emits library code only. No native foreign
execution, QEMU, firmware image, or target memory-fit claim follows from them.

The following remain explicit separate work:

| Contract | Current boundary |
| --- | --- |
| Rust 1.100 compatibility | Official installation probe returned HTTP 404; only the exact 1.101 nightly pin is verified. A source stability annotation is not an earlier-compiler run. |
| Custom shared outer storage | Standard Arc/Rc allocator APIs and fallible Box construction remain outside the inspected stable subset. Shared dictionary payloads are custom-owned; counts and inline object remain infallible Global storage. |
| Static caller workspaces | Typed custom allocator owners do not provide same-address context initialization, bounded no-heap arenas, or C static workspace contracts. |
| Size estimation | Reachable retained capacity excludes inline state and bookkeeping, and includes borrowed dictionary payload. It is neither C `sizeof` nor a guaranteed required-workspace estimator. |
| Automatic shrinking | Reset deliberately retains reusable capacity. The upstream repeated-small-frame shrinking policy is not implemented. |
| Stable caller buffers | Current process slices expire at call return; a future borrowed session must express longer buffer ownership and reuse guarantees. |
| Numeric levels and workers | The exact large allocator source campaign uses the three supported independent strategies, not upstream levels 1–6 or worker schedules 1–4. |
| Release qualification and complete upstream migration | Focused platform checks and deterministic campaigns do not complete native platform qualification, long-running fuzzing, or every upstream behavioral source unit. |

The [upstream allocation plan](upstream-allocation-plan.md) preserves the exact
original schedules for these gaps. No inventory entry is promoted merely because
its underlying codec allocation can now be selected by the caller.
