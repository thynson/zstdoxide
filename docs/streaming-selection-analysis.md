# Streaming selection lifecycle analysis

This is the first analysis item of Module 39. No selection owner or API is
implemented here. The pinned revision is facebook/zstd v1.5.7
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`. It extends
[the Module 38 lifecycle analysis](prepared-dictionary-streaming-analysis.md)
with prefix replacement, parameter-only resets and pledged-source window
updates.

## Reference-time mutual clearing

`ZSTD_CCtx_refCDict` (`zstd_compress.c:1328-1338`) is accepted only while
`streamStage == zcss_init`. It calls `ZSTD_clearAllDicts` (`:154-163`: frees
the local dictionary buffer and CDict, zeroes `prefixDict`, clears the
referenced CDict pointer) and then stores the CDict pointer.

`ZSTD_CCtx_refPrefix_advanced` (`:1351-1363`) uses the same init-stage gate and
the same `ZSTD_clearAllDicts`; it stores the prefix only when the pointer is
non-null and the size is nonzero, so re-referencing an empty prefix is a
detachment. `ZSTD_CCtx_refPrefix` (`:1346-1349`) is the raw-content form: it
passes `ZSTD_dct_rawContent`, which never contributes a dictionary ID or
entropy. A referenced CDict and a pending prefix can never coexist
(`:6357` asserts one of the two is null), so every reference call fully
replaces the selection.

## Reset directives

`ZSTD_CCtx_reset` (`:1365-1382`) distinguishes two scopes:

- `ZSTD_reset_session_only` returns the stage to `zcss_init` and clears the
  pledged source size. Requested parameters, a referenced CDict and a
  still-unclaimed prefix all survive; only claimed prefixes are gone because
  frame initialization consumed them earlier.
- `ZSTD_reset_parameters` and `ZSTD_reset_session_and_parameters` require
  `zcss_init` (`stage_wrong` otherwise), call `ZSTD_clearAllDicts` and reset
  the requested parameters to defaults through `ZSTD_CCtxParams_reset`
  (`:353-362`: default level 3, content-size flag on, everything else zero).

A completed frame resets only the session inside `ZSTD_compressStream2`, on
both the direct-in-output and the flush completion paths
(`:6241-6245`, `:6268-6273`; the worker path is `:6483-6502`). The next frame
therefore reuses a referenced CDict without a new reference call, while a
consumed prefix never comes back. `ZSTD_CCtx_setPledgedSrcSize`
(`:1230-1238`) is accepted only at `zcss_init` and only updates the pledge.

## Frame initialization

`ZSTD_CCtx_init_compressStream2` (`:6346-6443`) performs the per-frame
selection work:

1. It copies `prefixDict` into a local and memsets the stored copy — single
   usage (`:6354-6357`). A prefix applies to exactly one frame; the CDict
   stays selected.
2. A referenced CDict's compression level takes priority over the requested
   level unless the CDict came from `ZSTD_initLocalDict` (`:6361-6365`).
3. `endOp == ZSTD_e_end` promotes the pending input length to the pledge
   (`:6366`).
4. `dictSize` is the prefix size or the CDict content size (`:6368-6371`), and
   `params.cParams` are re-resolved per frame through
   `ZSTD_getCParamsFromCCtxParams(&params, pledge, dictSize, mode)` with
   `mode = ZSTD_getCParamMode(cctx->cdict, &params, pledge)` (`:6371-6375`,
   `:5959-5965`).
5. `ZSTD_compressBegin_internal` (`:5235-5290`) attaches or loads a CDict by
   the pledge cutoffs `ZSTD_USE_CDICT_PARAMS_SRCSIZE_CUTOFF` (128 KB) and
   `ZSTD_USE_CDICT_PARAMS_DICTSIZE_MULTIPLIER` (6) plus the
   `ZSTD_dictForceLoad` override (`:5253-5260`, `:2309`, `:2497`). A prefix
   always goes through `ZSTD_resetCCtx_internal` plus
   `ZSTD_compress_insertDictionary` (`:5191-5215`), which for raw content is
   `ZSTD_loadDictionaryContent`.

`ZSTD_loadDictionaryContent` (`:4898-4990`) truncates an oversized prefix to
the index-overflow cap and then to the table cap
`1 << min(hashLog + 3, chainLog + 1)`, loading only the suffix. The suffix
anchors the window before the frame, so prefix distances stay ordinary window
distances, and `loadedDictEnd` is zeroed only under `params->forceWindow`
(`ZSTD_c_forceMaxWindow`). A `ZSTD_dct_fullDict` prefix additionally loads
entropy and a dictionary ID; a raw prefix contributes neither.

## Pledged-source window updates

The per-frame re-resolution in step 4 is where a pledge changes the effective
window:

- `ZSTD_getCParamsFromCCtxParams` (`:1637-1652`) falls back to the requested
  `srcSizeHint` when the pledge is unknown, applies explicit cParams
  overrides, and finishes in `ZSTD_adjustCParams_internal`.
- `ZSTD_adjustCParams_internal` (`:1465-1545`) shrinks `windowLog` to
  `highbit32(pledge + dictSize - 1) + 1` when both values are known and below
  half the maximum window. In `ZSTD_cpm_attachDict` mode the dictionary size is
  zeroed first because the CDict carries dedicated parameters.
- `ZSTD_dictAndWindowLog` (`:1440-1462`) expands the window instead when a
  dictionary plus a small source would not otherwise cover both, so an
  attached CDict's distances remain representable.

The adjusted window is what the frame header declares. A smaller pledge can
therefore shrink the declared window of the next frame of a stream, and a
larger dictionary can grow it, without a new reference call.

## Mapping to the Rust streaming coordinator

The Module 38 `CompiledStreamEncoder` owns one compiled selection for the
whole stream and its profile is fixed at construction. The named-policy
`Encoder` already implements the upstream selection lifecycle for its own
engine: `set_dictionary` clears an unclaimed prefix, `set_prefix` clears the
persistent dictionary, frame initialization claims a prefix once
(`prefix.take()`), session reset keeps both pending selections, and
`reset_parameters` clears every handle at a clean boundary.

The bounded Module 39 shape gives `CompiledStreamEncoder` the same lifecycle
without a second streaming engine:

- One fixed profile from construction. Selection changes never re-resolve
  parameters; each compiled frame revalidates the fixed profile's window and
  staging admission instead, mirroring the upstream per-frame initialization
  in contract rather than in resolution.
- A selection owner holds either the owned `CompiledDictionary<'a, B>` or a
  borrowed one-frame `DictionaryView<'a>` prefix, never both, and never
  clones compiled tables. Replacing a compiled selection moves it back to the
  caller so tables are neither copied nor silently dropped.
- Frame initialization claims a pending prefix exactly once (upstream single
  usage at `:6354-6357`); a raw prefix contributes no ID and no entropy seed,
  a formatted prefix contributes both, matching the dct split. Prefix frames
  seed the per-call tables from
  `staged prefix | retained history | block` through the existing
  `ReferenceSearchState::new_with_prefix` path; this per-frame seeding is the
  prefix contract itself, while compiled tables are still never reseeded.
- `reset` stays session-only and keeps the compiled selection and an
  unclaimed prefix. A parameter reset requires a clean boundary, restores
  default frame flags, removes the selection and returns the compiled state
  to the caller; it never reseeds anything.
- After the prefix frame completes, the coordinator has no selection and
  frames continue dictionary-free under the same fixed profile, which is the
  upstream post-prefix state.

The six compiled families (Fast, DoubleFast, Greedy, Lazy, Lazy2,
BinaryTreeLazy2) all admit dictionary staging, so prefix and dictionary-free
frames stay inside the coordinator's fixed plan dispatch; the Optimal family
remains unreachable by construction.

## Pledged windows in the bounded contract

The Rust coordinator keeps the construction window for every frame. The
upstream pledge-driven shrink (`ZSTD_adjustCParams_internal`) and dictionary
expansion (`ZSTD_dictAndWindowLog`) are workspace optimizations and header
descriptor changes that this contract deliberately does not adopt: the
declared window stays the profile window, `pledged_size` remains a frame flag
controlling the header field and completion checks, and no pledge can move a
distance floor. This is a recorded deviation, not a claim of byte identity
with native `ZSTD_compressStream2`.

## Open decisions

- Exact names for selection replacement and whether replacing a prefix with a
  prefix returns anything.
- Whether a zero-length prefix detaches the selection like upstream or is
  rejected as an invalid parameter.
- The parameter-reset error boundary: clean Active boundary only, or also the
  Finished state that follows a completed frame.
- Whether dictionary-free frames after selection removal are a supported
  contract or a rejected state.
- Whether formatted-prefix entropy seeds the first block in streaming the same
  way the one-shot parameterized path already does.

The design item must decide these before code. Native replacement/reset
schedules, the feature matrix and timing remain separate qualification items.
