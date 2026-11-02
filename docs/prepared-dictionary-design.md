# Reusable prepared-dictionary compression state

Module 37 follows parameterized dictionary-backed one-shot compression. This
design and work decomposition precede implementation. The bounded scope is an
immutable, reusable compression state that owns the match-finder seed tables and
prepared entropy for one validated dictionary and one resolved reference
profile, so repeated one-shot calls do not restage or reseed dictionary content.
No streaming, native context lifecycle, long-distance matching, workers or
arbitrary frame settings are added.

## Upstream counterparts and ownership

The pinned facebook/zstd v1.5.7 revision
`f8745da6ff1ad1e7bab384bd1f9d742439278e99` implements this reuse as
`ZSTD_CDict`. `ZSTD_createCDict_advanced2` resolves parameters with
`ZSTD_getCParamsFromCCtxParams` and the `ZSTD_cpm_createCDict` mode, reserves
one owned workspace (`ZSTD_createCDict_advanced_internal`), and calls
`ZSTD_initCDict_internal` to insert dictionary entropy and content
(`zstd_compress.c:5551-5604`, `:5605-5710`). The resulting `struct
ZSTD_CDict_s` owns the dictionary content reference or copy, an entropy
workspace, one match-state workspace, one compressed-block previous state, the
custom allocator record, the dictionary ID and the selected parameters
(`zstd_compress.c:80-97`). `ZSTD_CCtx_refCDict` attaches it by reference
(`zstd_compress.c:1328`), and `ZSTD_compress_usingCDict_advanced` runs the
one-shot emission over the attached CDict (`zstd_compress.c:5884-5904`).

`ZSTD_initCDict_internal` loads entropy with `ZSTD_compress_insertDictionary`
and `ZSTD_loadCEntropy`, then calls `ZSTD_loadDictionaryContent`
(`zstd_compress.c:4898-4970`, `:5179-5220`). That loader truncates the loaded
suffix to the indexable range, sets `nextToUpdate` to the loaded start and
`loadedDictEnd` to the loaded end, and fills the selected match finder:
fast hash, DoubleFast double hash, chain/row tables or binary tree. The loaded
end is the window-expiry boundary; entries older than the resolved window may
not produce matches.

In Rust, a `CompiledDictionary<'a, A>` owns only:
- the resolved scalar profile (window/hash/chain/search logs, minimum match,
  target length and strategy) used to build the tables;
- per-profile owned seed tables from the same family as the per-call parsers
  (`unsafe-match-table`, `unsafe-reference-chain`, `unsafe-row-table`,
  `unsafe-reference-tree` keep their independent opt-in implementations);
- prepared literal/sequence entropy only when the dictionary is formatted;
- the dictionary ID and a borrow of immutable dictionary content for match
  extension.

It owns no source, destination, frame output or live encoder. Construction takes
an immutable `DictionaryView<'a>` and a `CompressionParameterRequest` and
returns a state that borrows the view for its whole lifetime. Sharing uses
`Arc<CompiledDictionary>` when callers need it; cloning the handle never clones
tables or content. All table and entropy ownership stays in allocator `A` and is
released exactly once. Equal-output claims are limited to paths qualified with
exact comparisons; the state must not silently substitute a strategy or drop a
dictionary.

## Proposed public contract

Provide construction from a borrowed or shared dictionary plus a request, with
admission identical to Module 36: Fast, DoubleFast, Greedy, Lazy, Lazy2 and
BinaryTreeLazy2 when the dictionary is nonempty; Optimal, Ultra and Ultra2 are
rejected with the existing unsupported-feature error. Empty and raw
dictionaries preserve the dictionary-free parameterized bytes where the
existing contract does.

Provide one-shot caller-owned and allocating emission through the compiled
state, plus a coordinator selection variant. The request is resolved against
the actual source length and the dictionary's matchable content length at each
call; the compiled profile is fixed at construction. A source that resolves to
a different profile than the compiled one may either be rejected explicitly or
resolved through the existing per-call path with a documented rule; it must
never use mismatched tables. Destination capacity, guard and partial-append
behavior match `compress_into_with_parameters_and_dictionary`. Dictionary
window expiry uses the constructed profile's resolved window and the loaded
suffix boundary, not the caller's later request. Prepared entropy seeds only
the first emitted block and only for formatted dictionaries, matching Module 36.

The [reuse geometry refinement](prepared-dictionary-analysis.md#reuse-geometry-refinement)
fixes the internal shape: compiled tables stay immutable during calls, hold
content-relative positions, and are consulted as a second read-only candidate
source beside per-call tables that alone receive input sampling. A call stages
exactly the compiled `loaded_len` suffix before its input, so the staging delta
is zero and positions already coincide; a future call that stages less must
translate by the delta and floor every candidate at it. The merged read takes a
nonempty per-call entry first and the compiled entry otherwise, which reproduces
a table seeded with dictionary positions and then overwritten by call sampling.
Byte equality with Module 36's combined-staging path is still a measured
per-family comparison recorded in the evidence rather than a general contract,
because candidate streams, sampling and compilation order remain separate
mechanisms even where a deterministic pair matches exactly. Raw and empty
dictionaries never build compiled tables and keep the exact dictionary-free
parameterized bytes; the compiled profile must never silently substitute a
strategy or drop a dictionary.

## Qualification before promotion

The pinned loading pipeline, sampling frontier, loaded-end expiry and
by-reference versus by-copy ownership are analyzed first in
[the compiled-dictionary analysis](prepared-dictionary-analysis.md). Check every
admitted strategy at minimum, default and maximum boundary levels with empty,
raw and formatted dictionaries: exact output comparison to the existing
combined-staging bytes, unchanged dictionary content, guards/suffixes,
allocation-failure release, compiled-state reuse across repeated calls and
independent allocator domains. Reject the optimal family before writes. Compare
guarded native CDict schedules resolved from the same levels through the pinned
C library without claiming native compressed-byte identity, capacity
precedence or workspace equivalence. Qualify under concrete no-atomics ARM code
generation, running foreign Miri only if this module adds or modifies unsafe
code. Then run the complete feature/release matrix, and compare compiled-state
against per-call-staging workloads with explicit setup placement and
independent repeats after CPU barriers.

## Exclusions

Native `ZSTD_CCtx` parameter lifecycle, reset directives, dedicated dictionary
search, long-distance matching, workers, streaming and complete upstream
parity remain out of scope. A compiled dictionary is not a live compressed
context and retains no input, output, frame or attempt state.
