# Streaming selection lifecycle design

This is the Module 39 implementation decision recorded before code. It follows
[the lifecycle analysis](streaming-selection-analysis.md) and keeps the pinned
v1.5.7 revision `f8745da6ff1ad1e7bab384bd1f9d742439278e99` as the reference.

## Decision

The selection lifecycle is added to the existing `CompiledStreamEncoder`
in place. A second streaming coordinator, a wrapped `Encoder` or a generalized
prefix-only constructor would duplicate the reference block engine or invent a
parameter source the module does not qualify. Construction still requires one
compiled dictionary: it fixes the seven-field profile, window and block
geometry for the coordinator's whole life, exactly as in Module 38. Selection
changes replace *which dictionary seeds frames*, never the profile.

The coordinator stores three fields instead of the single owned compiled state:

```text
compiled:       Option<CompiledDictionary<'a, B>>   // selected compiled dictionary
prefix:         Option<DictionaryView<'a>>          // pending, unclaimed prefix
active_prefix:  Option<DictionaryView<'a>>          // claimed at frame init
```

Invariant: `compiled` and `prefix` are never both `Some`. Every prefix is a
borrowed `DictionaryView<'a>` over the caller's storage with the same `'a` as
the compiled content borrow; no compiled table, entropy view or dictionary
content is ever cloned. The active prefix is not a second borrow: it is moved
out of `prefix` at claim time and returned or dropped at the frame boundary.

## Public contract

- `selection(&self) -> Option<DictionaryView<'a>>` replaces the Module 38
  `dictionary()` accessor: it reports the compiled view, the pending prefix or
  the active prefix, and `None` after selection removal. `parameters()` keeps
  reporting the fixed profile.
- `set_compiled(&mut self, compiled: CompiledDictionary<'a, B>)
  -> Result<Option<CompiledDictionary<'a, B>>>` corresponds to
  `ZSTD_CCtx_refCDict`: clean configuration boundary only; it clears any
  pending prefix and returns the previously selected compiled state. The new
  state's parameters must equal the construction profile, otherwise
  [`Error::InvalidParameter`] precedes every mutation. This keeps window,
  block limit and per-frame plans fixed; native admits any CDict here, and
  that is a recorded bounded deviation.
- `set_prefix(&mut self, prefix: DictionaryView<'a>)
  -> Result<Option<CompiledDictionary<'a, B>>>` corresponds to
  `ZSTD_CCtx_refPrefix_advanced`: clean configuration boundary only; it
  clears the compiled selection and any prior prefix and returns the
  displaced compiled state so tables are neither copied nor dropped. A
  zero-length prefix detaches the selection exactly like the upstream empty
  `refPrefix` (`zstd_compress.c:1357-1362`). The prefix's own content kind
  decides ID and entropy contribution; the upstream forced content-type
  parameter has no counterpart.
- `reset()` stays session-only: it clears the pledge and frame state including
  the active prefix, keeps the compiled selection and keeps an unclaimed
  prefix pending, mirroring `ZSTD_CCtx_reset(session_only)` keeping
  `prefixDict` until frame initialization consumes it.
- `reset_with_options(options)` replaces frame flags over an intact selection
  and pending prefix; nothing else changes.
- `reset_parameters(&mut self) -> Result<Option<CompiledDictionary<'a, B>>>`
  corresponds to `ZSTD_reset_parameters`: it is accepted at a clean boundary
  or after a completed frame (`Encoder::reset_parameters` boundary), restores
  `CompiledStreamOptions::default()`, clears the pending and active prefix and
  the compiled selection, and returns the compiled state. Workspace
  allocations and the fixed profile are retained. Two deviations are recorded:
  C also resets requested parameters to defaults, which would move this
  coordinator's fixed geometry, and C frees instead of returning the state.
  No table is reseeded by or after this call.
- `reset_all(&mut self) -> Option<CompiledDictionary<'a, B>>` corresponds to
  `ZSTD_reset_session_and_parameters`: every phase including a poisoned one is
  accepted, the session and selection are cleared, and the compiled state is
  returned.

The upstream `zcss_init` gate allows re-referencing a dictionary directly
after a completed frame because `ZSTD_compressStream2` resets the session at
frame end. The Rust family requires the explicit `reset()` first, matching
`Encoder`; this boundary difference is already established and is kept.

## Frame initialization and per-block dispatch

`start_header` claims the selection before any header byte: a pending prefix
is moved to `active_prefix` (single usage, `zstd_compress.c:6354-6357`) and a
compiled selection stays selected. The same call performs the fixed-profile
revalidation for compiled frames as scalar checks before the header: window
log inside the supported range, loaded suffix within the window and the
content borrow, matching the construction gate. A prefix frame checks the
staging cap arithmetic. Failures poison the stream like any processing error.

Each sealed block binds its parser by selection:

- Compiled: the Module 38 path unchanged —
  `suffix(loaded_len) | history | block` with `new_with_compiled`; compiled
  tables are never reseeded.
- Prefix: `staged prefix suffix | history | block` with `new_with_prefix`,
  where the staged suffix is the last `min(prefix.len(), window)` bytes. This
  is a per-block seeding pass, which mirrors upstream re-loading a prefix at
  every frame initialization; within one native frame the loaded prefix
  persists across blocks, so per-block re-seeding is a recorded workspace
  deviation, not a semantics change. Prefix candidates obey the same window
  floor argument as compiled suffix candidates: before the window wraps the
  local distance equals the frame distance, and after the wrap both exceed the
  declared window and the parser floor rejects them.
- No selection: `history | block` with `new_with_prefix` and the history as
  prefix length, the ordinary dictionary-free streaming binding. Frames after
  selection removal are a supported contract under the fixed profile, matching
  the upstream post-prefix state.

Frame headers carry the active selection's nonzero ID when
`include_dictionary_id` is set; a raw prefix and an empty selection write no
ID. A formatted prefix's prepared entropy seeds the first block through the
existing `start_frame` path exactly as the compiled seed and the one-shot
parameterized dictionary path already do. The declared window is always the
profile window; a pledge never re-resolves it (the analysis records the
upstream `ZSTD_adjustCParams_internal`/`ZSTD_dictAndWindowLog` shrink and
expansion as the adopted-against deviation). Everything else — the process
latch and poison contract, checksum, history retention, RLE gating, per-block
plan rebuild, output drainage and allocator-failure release — is the Module 38
contract verbatim and is not restated here.

## Ownership and lifetimes

`CompiledDictionary<'a, B>` moves in and out of the coordinator by value and
is never cloned; dropping it or handing it back releases no borrowed storage
because the dictionary content lives in the caller's `'a`. A prefix view is
copyable metadata over the same `'a`. The workspace `Vec<u8, A>` owners and
the entropy encoder are unchanged by selection transitions; `retained_capacity`
keeps excluding dictionary-owned storage. Debug output reports the selection
phase. No new unsafe code and no Miri obligation arise.

## Explicit non-goals

Prefix-only construction without a compiled identity, per-frame cParams
re-resolution or pledged window changes, forced prefix content types,
persistent (multi-frame) prefixes, native attach-versus-load workspace reuse,
LDM, workers, numeric levels and byte identity with native prefix frames are
not claimed. Rust prefix frames are decoder-valid; they are not expected to
match native `ZSTD_compressStream2` bytes.

## Qualification plan

Unit tests cover claim-once semantics, mutual replacement with intact compiled
state, detach by empty prefix, session reset keeping an unclaimed prefix,
parameter-reset boundaries and defaults, reset-all recovery, formatted-prefix
ID and entropy seeding, dictionary-free frames after removal, allocator
failure across every selection transition, and single-block body equality with
the one-shot compiled frame on the compiled path. Public integration tests
exercise the six families across replacement sequences and cross-check every
emitted frame against the Rust decoder and the pinned native decoder. Native
schedules pin reference replacement and reset directives, including stage
errors. The feature/release/no_std/ARM matrix, strict Clippy and the
selection-reset versus replacement timing campaign close the module as
separate items.
