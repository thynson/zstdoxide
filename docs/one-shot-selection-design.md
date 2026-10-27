# Reusable one-shot dictionary selection

Module 32 follows scalar bounds/skippable writing. Record ownership and its TODO
before implementation. Promote only after Module 31 closes. This is a bounded
selection/lifecycle coordinator over existing caller-owned-output decoders; it
is not an opaque native context or retained decoder-scratch workspace.

## Pinned behavior and reason for a separate coordinator

facebook/zstd v1.5.7 revision `f8745da6ff1ad1e7bab384bd1f9d742439278e99`:
`ZSTD_getDDict` and `ZSTD_decompressDCtx` in `lib/decompress/zstd_decompress.c`
lines 1180-1200 claim a use-once dictionary at call entry. The selected DDict is
then passed once to `ZSTD_decompressMultiFrame` and reused for every standard
frame in that call. `ZSTD_DCtx_refPrefix_advanced` at line 1725 installs a borrowed
raw/auto-content dictionary with use-once lifetime. This differs from incremental
frame consumption and from the existing SliceDecoder prefix policy.

A prefix must therefore remain usable for two dependent standard frames within
one call, then disappear before the next call, even when the first call fails or
contains only skippable/empty input. Do not implement this with the incremental
prefix constructor or silently reinterpret one-shot persistent wrappers.
Native empty-input acceptance and stricter Rust dictionary-window checks remain
explicit format-policy differences. Native context reset/parameter/workspace
semantics are separate and must not be inferred from this small coordinator.

## Ownership and invariants

Add `OneShotDecoder<D = SharedDictionary>` containing copied DecodeOptions and
one private selection enum: no dictionary, persistent D or next-call D. Exactly
one selection exists. The generic sealed DictionaryHandle supplies immutable
DictionaryView; do not require Clone or copy dictionary bytes. Shared owners and
borrowed views retain their established independent allocator/lifetime contracts.
No input or destination reference is stored in the coordinator.

Claiming selection creates a transient call record: no dictionary, a borrow of
the persistent owner, or ownership of the removed next-call owner. Claim next-call
selection before invoking any decoder/preflight/allocator operation. The local
record keeps that owner alive throughout every frame in the call and drops it
on return or unwinding. The coordinator already has no next-call selection, so
an error cannot restore it accidentally. Persistent selection remains unchanged.
Borrowed views cannot outlive their original prepared/raw dictionary owners.

The enum and call record are plain ownership/state records with no raw-memory
operations or independent hot storage. Share their representation across all
safe/unsafe configurations, without artificial unchecked features. Existing
output/history/cursor/dictionary backends remain independently selected. Keep
the module no_std plus alloc; default constructors use Global only through the
existing one-shot wrapper. A caller-supplied scratch allocator is passed per call
and never retained. Scratch allocations remain fresh and fallible; workspace reuse
is not claimed. Dictionary owner drops/clones, if performed by callers, retain
ordinary Rust allocator/panic semantics; the coordinator never clones a handle.

## Public contract

- `new(options)` constructs the default shared-owner type with no selection.
  Generic constructors accept an owned persistent handle or an owned next-call
  handle. A generic empty constructor permits later installation of borrowed views.
- `set_dictionary(Option<D>)` replaces selection with persistent ownership or clears
  it. `set_prefix(Option<D>)` installs next-call ownership or clears it. Replacement
  drops the prior handle without cloning or allocating. There is no in-progress
  asynchronous phase; exclusive mutable access prevents concurrent calls/updates.
- `dictionary()` exposes only an immutable view of the current selection;
  `prefix_pending()` reports next-call status. Options have copied getter/setter
  access. Avoid an opaque reset API: explicit option/selection replacement has
  clear Rust ownership and does not claim all native reset directives.
- `decompress_into(input, output)` and `decompress_into_in(..., allocator)` reuse
  the existing direct output engine. The chosen dictionary is persistent across
  this call's frames, whether the stored selection was persistent or next-call.
- `decompress_in_place(buffer, input_start)` and its allocator variant reuse the
  existing suffix-input engine and its layout/guard contract. The selected
  dictionary remains independent of the exclusive buffer borrow.

Every decode attempt claims next-call selection, including empty input, invalid
suffix coordinates, capacity/window failure, checksum/entropy failure and scratch
allocation failure. Invalid Rust suffix coordinates have no safe native pointer
analogue; consumption on that attempt is an explicit Rust API rule. Persistent
selection survives all such errors. A fresh call has fresh frame/history/entropy
state and needs no poison-reset operation. Resource limits and provisional output
contracts remain those of the delegated decoder; this coordinator promises no
additional error atomicity. Prefix ownership is dropped after the attempt, even
when no standard header is reached. Zero-length dictionaries retain their handle
lifetime policy but do not invent content or dictionary dependence.

## Qualification and scope

Verify selection transitions independently before exposing the public type.
Exercise real raw/prepared dictionary dependence, two dependent frames in one
call, intervening skippables, separate subsequent calls, empty/skippable-only/
malformed input and every failure family above. Test direct and actual shared-
buffer in-place modes, exact output/tails, independent dictionary/scratch allocators,
non-Clone borrowed owners and handle-drop accounting. Compile-fail examples prove
borrowed dictionary lifetime and exclusive output/context access.

Use normal and ASan/UBSan v1.5.7 context helpers with actual DCtx refPrefix/raw/auto
and persistent DDict transitions. Compare complete output and observed subsequent
calls, not just the first success. Keep true capacity guards and actual in-place
source/destination pointers. Report empty/window/error-order differences explicitly.
Generated lifecycle schedules supplement original assertions; original upstream
group status changes need their complete preceding history and exact obligations.

Run focused checks, foreign i686/s390x Miri, actual no-atomics core+alloc objects,
all isolated/all/safe full and release suites, no_std and strict Clippy. Freeze
identical persistent/prefix workloads with dictionary/scratch ownership ledgers,
separate setup/restoration costs and independent safe/all rebuilds and timing.
No whole-context workspace reuse, arbitrary C parameter/reset equivalence,
incremental prefix change or complete upstream parity is claimed. Reconcile the
bounded milestone and decompose the next contract only after actual gates pass.
