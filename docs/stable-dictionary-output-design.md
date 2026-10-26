# Persistent dictionaries in stable borrowed output

Module 26 design is recorded before candidate implementation. Predecessor
validation sources remain frozen. Promote this candidate only after Module 25
closes; this design does not claim implementation or admission.

Preserve `SliceDecoder::new` / `new_in` and existing default type annotations by
adding a third defaulted parameter `D = SharedDictionary`, matching the buffered
Decoder's sealed handle approach. The record becomes
`SliceDecoder<'output,A=Global,D:DictionaryHandle=SharedDictionary>` and owns an
`Option<D>` in addition to existing state. No self-referential table/view is stored.
Constructors `with_dictionary(output,options,dictionary)` and
`with_dictionary_in(output,options,dictionary,allocator)` consume the selected
shared handle or borrowed view. The former uses Global scratch; the latter
allows independently allocated dictionary and scratch owners. Construction need
not clone the supplied attachment. The normal constructors store None and retain
the current clone/allocation behavior. No dictionary replacement during a
session is necessary in this first bounded milestone.

At header admission, nonzero IDs require a matching configured view. ID zero
still uses the configured content and entropy. Seed repeat offsets per standard
frame, borrow prepared entropy only during each block call and keep owned table
fallback/reset semantics unchanged. Concatenated frames reuse the persistent
attachment; skippable frames do not consume it. `reset()` preserves attachment,
output borrow and options while dropping frame-local Huffman/FSE overrides,
retaining the sequence allocator and clearing encoded input length as before. Dictionary-free constructors and all
existing progress/error/poisoning behavior must remain unchanged.

`SliceHistory` remains the sole destination and position owner. For sequence
execution, an ephemeral adapter can borrow `&mut SliceHistory` and a temporary
immutable dictionary view across only that decode call. This avoids moving the
history out of the session or storing an interior reference into the attachment.
It delegates append/length and invokes the existing checked dictionary-match
operation. Do not create another unsafe storage family for this reference/index
record. Both existing unsafe-slice-history and unsafe-dictionary features remain
independent. The dictionary-to-frame crossing invariant and documented expiry
rule from Module25 remain exactly the same.

Preserve `retained_capacity()` as codec-owned scratch capacity, excluding output
and dictionary payload, exactly as for existing sessions. Add
`dictionary_retained_capacity()` for payload reachable through the attachment,
zero when absent. It does not claim ownership of shared/borrowed payload or count
it twice when another context shares the same owner. Tests must show raw/RLE
session construction and processing do not allocate codec output, dictionary/table
ownership is unchanged, borrowed nonclone allocators retain their original
identities and every scratch allocation failure releases or resets correctly.

Tests should use exact/short fixed destinations over all independent prepared
and raw dictionary fixtures, every split of a representative small prepared
frame, bytewise and seven-byte inputs, persistent concatenation/skips, reset after
success/failure, invalid/missing/mismatched IDs, the exact window boundary and
crossing into earlier frame output. Compare valid output with independent
originals and the buffered dictionary decoder. Keep lifetime compile failures
for destination mutation and for a dictionary view escaping its owner. Test
that a consumed shared handle keeps its dictionary alive until session drop.

Native normal/sanitized helpers combine ZSTD_d_stableOutBuffer with refDDict or
loaded dictionary setup and actual guarded outputs. Runtime C buffer mutation
remains excluded by Rust borrowing. Existing trained dictionary stream fuzz
output schedules are not stable-capacity schedules: any forced-stable supplement
must be labelled, preserve its own source/dictionary selections, and must not be
counted as an exact port of the original changing-output target. Prefixes,
attachment replacement/reset-parameter APIs and C workspace contracts remain
separate scopes.

Full safe/isolated/all regression, release, strict Clippy, no_std, foreign Miri
and concrete ARM generation precede timing. Compare the same persistent
dictionaries and whole/seven-byte fragment schedules against buffered decoding.
Dictionary preparation and caller destination allocation stay outside clocks;
state construction/scratch/destruction remain inside for cold-session results.
Retained-session performance needs a separate explicitly scoped campaign.

Qualification decomposition:

1. Finalize generic attachment, reset/lifetime and separate capacity contracts.
2. Introduce an ephemeral borrowed dictionary sequence sink and verify safe,
   isolated slice/dictionary, combined and all backends on crossing boundaries.
3. Integrate private per-frame dictionary IDs, repeat offsets and prepared-table
   fallback while preserving dictionary-free allocation/progress behavior.
4. Expose persistent shared/view session constructors with borrowed/nonclone
   allocators, lifetime examples, failure poisoning/reset and ownership checks.
5. Independently qualify normal/sanitized native stable dictionary destinations,
   actual guards and the explicitly different p>window policy.
6. Add a clearly labelled forced-stable persistent dictionary corpus supplement;
   report original prepared/loaded/prefix branch counts and retain buffered prefix
   behavior rather than claiming a new exact original stable branch.
7. Full safe/isolated/all/release/no_std/Clippy, foreign Miri and concrete ARM.
8. Frozen whole/seven-byte dictionary workloads, clock-free allocation ledgers,
   retained safe/unsafe binaries and an independent repeat after correctness.
9. Reconcile bounded coverage and decompose single-use borrowed prefixes.

## Ownership and representation

| Record | Ownership and invariants | Upstream counterpart |
| --- | --- | --- |
| Persistent attachment | `Option<D>` owns a shared handle or a borrowed view; its existing owner supplies immutable bytes and entropy | Referenced or loaded DDict selection in ZSTD_DCtx |
| Borrowed sequence sink | Temporary `&mut SliceHistory` plus immutable `DictionaryView`; no allocation, raw pointer, or self-reference | Stable-output dictionary sequence execution |
| Initial entropy | A copied view computed from the attachment for one block call; never stored alongside its owner | DDict entropy fallback and repcodes |
| Scratch and history | Existing encoded buffer, Huffman/sequence owners and exclusive output borrow; frame bounds unchanged | Stable destination prefix and bounded input buffer |

All new records have safe shared representations. Actual content/destination
access uses the independently selected unsafe-dictionary and unsafe-slice-history
backends from Module 25, with the same validation and safety contracts. The
default attachment type follows the existing SharedDictionary Arc/Rc selection;
auto traits follow the selected dictionary handle and allocator. Source chunks
remain temporary borrows and are never retained.

Header admission must finish dictionary ID validation before window/content
checks. Each standard frame reinitializes repeats from the selected dictionary,
including after reset and after intervening skippable frames. Owned working
tables replace borrowed fallback only within that frame. Completing or resetting
a frame releases owned Huffman/FSE overrides while retaining the sequence
allocator and encoded-input capacity. A failed session retains its persistent
attachment until reset/drop and denies output access, as before.

The exact source-prefix and dictionary selectors of dictionary_decompress remain
preserved in the supplemental corpus test. Its fixed seven-byte input schedule
is newly chosen for stable sessions and is explicitly not a selector from that
one-shot native target. Keep the original direct-output corpus run separate so
its old evidence is not silently relabelled as streaming qualification.
