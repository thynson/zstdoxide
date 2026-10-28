# Call-scoped dictionary selection for caller-owned compression

Module 35 follows dictionary-backed caller-owned compression. This design and
work decomposition precede implementation; promote implementation only after
Module 34 closes. The bounded scope is reusable dictionary selection around the
existing ordinary Greedy one-shot encoder with known size and checksum.

## Ownership and upstream counterparts

Pinned facebook/zstd v1.5.7 revision
`f8745da6ff1ad1e7bab384bd1f9d742439278e99` distinguishes persistent CDict selection
from a single-use prefix. `ZSTD_CCtx_refCDict` and `ZSTD_CCtx_refPrefix_advanced`
replace one another. `ZSTD_compress2` resets session state, invokes a finishing
compression operation and restores its temporarily selected stable buffer modes.
`ZSTD_CCtx_init_compressStream2` copies and clears prefix selection before frame
initialization. The local-dictionary preparation call precedes that clearing;
our immutable, preconstructed handles have no equivalent lazy preparation step.
Do not infer identical native error precedence or workspace lifetime.

Reuse the private `Selection<D>` and `Claimed<'_, D>` ownership records already
qualified for one-shot decoding. Selection owns no input/output/scratch; it owns
one persistent or next-call handle, or nothing. Claiming a persistent handle
borrows it. Claiming a prefix moves it into a call-local owner and changes stored
selection to None before output admission or allocator work. The owner stays
alive until the attempt returns or unwinds. Plain enums use the same safe
representation under every backend; no artificial unsafe feature is needed.

A new `OneShotEncoder<D = SharedDictionary>` contains only selection. A borrowed
DictionaryView can be its D and cannot outlive the dictionary owner. The existing
sealed DictionaryHandle supertrait includes Clone, but this coordinator's
transitions and calls must never clone that handle. A copied view borrows the
immutable bytes, identifier and prepared entropy; no dictionary history is copied.
Each attempt delegates to the existing caller-owned encoder with a fresh scratch
allocator and existing independently selectable safe/unsafe storage components.

## Proposed public contract

Provide new/without_dictionary, with_dictionary/with_prefix, set_dictionary,
set_prefix, dictionary and prefix_pending in parallel with OneShotDecoder.
Provide compress_into and compress_into_in, using Global or a supplied scratch
allocator. No compression options are stored: reference levels, arbitrary frame
settings, streaming, workspace retention, context reset directives and parameter
lifecycle are separate contracts. Do not add those merely to imitate a C context.

A prefix is consumed by every attempted compress_into call, including empty input,
zero/short output, allocation error and unwinding. Persistent selection survives
attempts until explicitly replaced or cleared. Input/output/allocator borrows do
not survive a call. All output bounds, provisional append behavior, exact existing
Rust frame bytes and suffix guarantees are inherited from Module 34 or ordinary
compression when no selection is present. Dictionary ownership and scratch
allocation domains remain independent, including borrowed non-Clone allocators.

Interpretation is fixed when constructing a handle. Dictionary::raw corresponds
to ordinary native refPrefix raw content; a formatted handle corresponds to the
advanced content-type contract. No implicit raw reinterpretation is performed.
The default C refPrefix API and its advanced variant must not be conflated.

## Original assertion boundaries

Pinned tests/fuzzer.c contains replacement checks labelled "Loading a dictionary
clears the prefix", "Loading a cdict clears the prefix", "Loading a prefix clears
the dictionary" and "Loading a prefix clears the cdict". Consult their existing
manifest status and preserve the exact dictionary/source preparation before
claiming a newly ported original assertion. A synthetic lifecycle schedule is a
supplement even when it exercises the same selection transition.

The nearby "testing ldm dictionary gets invalidated" group also fixes a 1 KiB
window, enables several long-distance controls, disables content size and covers
workers/streaming. Those branches remain unsupported by this coordinator. The
prefix determinism group varies levels and LDM and requires deterministicRefPrefix
with adjacent versus independent source storage. Ordinary Greedy equality on a
small copied fixture must not be called a complete port of either group.

## Qualification before promotion

Check successful, empty, capacity-error and allocation-error attempts, replacement
and clearing, repeated persistent calls and recovery after prefix consumption.
Compare exact complete bytes with the existing stateless encoder using the same
borrowed view. Check output guards/tails, dictionary bytes, retained handle counts,
every observed selected scratch failure, independent non-Clone owner domains and
unwinding. Compile-fail examples establish owner lifetimes and nonoverlap.

Native normal/sanitized schedules should compare subsequent native call results
with fresh native no-prefix and selected-prefix controls using identical options.
For raw-prefix evidence, first prove the chosen fixture actually changes the
native encoded result; otherwise byte equality cannot establish consumption.
For formatted selections, retain identifier and decode evidence too. Compare
Rust/native full-byte interoperability and subsequent selection outcomes without
requiring Rust/native compressed-byte identity, capacity choices, output suffix
writes, workspace allocation counts or all context semantics to match.

Reuse the deterministic dictionary selection supplements with explicit persistent
versus prefix attribution. This does not port original randomized parameters,
reference-level compression or all external rolling corpora. Qualify actual
selection/encoding under i686/s390x Miri and concrete no-atomics ARM emission,
every isolated feature and all/default configurations, no_std, strict Clippy,
formatting and release regression on a frozen source.

Performance comparisons must separate selection setup/consumption from stateless
borrowed dictionary calls, retain identical input/dictionary pairs and output
capacities, and account independently for dictionary, one-block staging and
workspace allocations. Prefix setup needed for each repetition must be explicit
in the timed operation or measured separately with a clearly stated combined
cost. Retain all executables/raw samples and an independent safe/all rebuild,
with fresh CPU barriers after qualification work completes.
