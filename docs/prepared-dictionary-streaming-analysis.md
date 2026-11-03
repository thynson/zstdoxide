# Streaming compiled-dictionary reuse analysis

This is the first analysis item of Module 38. No streaming compiled state or API
is implemented here. The pinned revision is facebook/zstd v1.5.7
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`.

## Pinned lifecycle

`ZSTD_CCtx_refCDict` (`zstd_compress.c:1328-1338`) is accepted only while
`streamStage == zcss_init`; it clears every other dictionary/prefix attachment
and stores the CDict pointer. `ZSTD_CCtx_refPrefix_advanced` uses the same
init-stage gate and clears all attachments, so a persistent CDict and a
one-frame prefix replace one another.

`ZSTD_CCtx_reset(ZSTD_reset_session_only)` (`:1365-1381`) returns the context to
`zcss_init` and clears the pledged source size but keeps the CDict and requested
parameters. `ZSTD_reset_parameters` and `ZSTD_reset_session_and_parameters`
clear every dictionary/prefix attachment. A completed frame in
`ZSTD_compressStream2` resets only the session
(`zstd_compress.c:6230-6275` and `:6460-6505`), so the next frame reuses the
same CDict without a new reference call.

At frame initialization `ZSTD_CCtx_init_compressStream2`
(`:6349-6443`) copies the pending prefix, clears `cctx->prefixDict` for single
use, lets a referenced CDict's compression level take priority, resolves
`params.cParams` with the CDict content size, and calls
`ZSTD_compressBegin_internal`. The CDict path may attach the immutable CDict
match state or load its tables into the context depending on source size and
attach preferences (`ZSTD_resetCCtx_usingCDict` /
`ZSTD_resetCCtx_byAttachingCDict`, `:2300-2520`). The frame header ID and loaded
content size come from the CDict. Prefix and CDict are mutually exclusive at
initialization (`:6357`).

## Mapping to the Rust streaming encoder

The existing `Encoder<A, D>` owns a persistent dictionary handle and an optional
one-frame prefix. Its `reset` preserves selection and `reset_parameters` clears
it, matching the upstream session/parameter reset split. Its frame engine builds
per-frame search tables and seeds dictionary content through the ordinary
Greedy path.

Compiled reuse needs a borrowed immutable state, not another owned dictionary
handle: the compiled tables live for the whole stream and must never be reset,
reseeded or mutated by a frame. The current `Encoder<A, D>` type has no lifetime
parameter for such a borrow. A separate coordinator is the smallest shape that
does not infect the existing generic API:

- `CompiledStreamEncoder<'a, A>` owns the existing `Encoder<A>` with no
  persistent dictionary plus a borrowed `CompiledDictionary<'a, B>` and a
  frame-level state machine.
- Construction validates the compiled profile before any frame. It borrows the
  compiled state; no table, entropy view or dictionary allocation is cloned.
- `compress`/`finish` delegate input/output and progress to the existing
  streaming engine while the engine consumes the compiled candidate views and
  first-block entropy seed, as the one-shot compiled frame path already does.
- A completed frame resets only the engine session; the compiled state stays
  selected for the next frame. A parameter reset clears the coordinator's
  compiled selection and returns to an empty stream.
- A one-frame prefix selection, if admitted, must clear/replace the compiled
  selection at frame initialization; the two cannot both seed one frame.
- The fixed compiled profile is not re-resolved per frame. Native can adjust
  window/parameters from a pledged source size in some attach modes; this
  bounded Rust contract keeps the compiled seven-field profile and rejects a
  caller configuration that would require another profile. It does not claim
  native attach/load decisions or workspace reuse.

## Window and frame boundaries

The compiled loaded suffix and its resolved window are fixed at construction.
Every frame stages exactly that suffix before its input, so candidate positions
remain content-relative and window expiry is the existing candidate floor.
Frame boundaries reset output/history/entropy but never the compiled tables.
The dictionary ID is written on every frame that uses the compiled selection;
a prefix frame has no compiled ID unless its own dictionary supplies one.

## Open decisions

- Exact public names and whether the coordinator exposes `flush` separately or
  only a single `compress` with `EndDirective`.
- Whether `reset_parameters` belongs on the coordinator or only on the wrapped
  encoder, and how a prefix selection interacts with the compiled lifetime.
- Whether a source pledge may change the effective window; if not, the contract
  must reject or document the ignored pledge.
- How to qualify native attach-versus-load differences without claiming
  byte/workspace identity.

The implementation item must decide these before code. The next qualification
items keep guarded native schedules, atomic-free ARM emission, the feature
matrix and streaming timing separate.
