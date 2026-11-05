# Reference-stream parameter lifecycle analysis

Prepared from pinned facebook/zstd v1.5.7
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`; line references are to
`lib/compress/zstd_compress.c` unless stated otherwise. This is source analysis,
not an implementation or a completed official test port.

## Requested versus applied parameters

`ZSTD_CCtx_setParametersUsingCCtxParams` (1178-1189) requires `zcss_init`,
rejects an attached CDict, and copies the requested record. It does not rebuild
workspace or resolve a source-specific profile. Despite the preceding comment
mentioning ongoing multithreaded updates, the function's actual gate rejects
non-init stages. Bulk request replacement is the appropriate analogue for a
Rust `CompressionParameterRequest` replacement.

`ZSTD_CCtx_setParameter` (708-718) permits some individual cParams changes
outside init by setting `cParamsChanged`; window, frame flags, source hints
and many other parameters are not authorized there (653-706).
`ZSTD_compressStream2` forwards changed requested parameters to an active
multithreaded job owner at 6487-6493. This is not a justification for mutating
this crate's already initialized single-threaded block geometry. Module 41
should retain clean-boundary replacement rather than adopt live per-field
updates or claim multithreaded behavior.

`ZSTD_CCtx_setPledgedSrcSize` (1230-1237) also requires init, but stores the
pledge directly in the context, outside `requestedParams`. `ZSTD_CCtxParams_reset`
(353-355) calls init at default level three; init zeroes the requested record,
restores its level and sets content-size emission to one (358-364). Checksum
therefore defaults off. The Rust request/options defaults already match those
supported scalar defaults.

## Three reset scopes

`ZSTD_CCtx_reset` (1367-1383) first handles the session scopes by returning to
init and clearing `pledgedSrcSizePlusOne`. The parameter scopes require init,
clear all dictionaries/prefixes and reset the requested record. Consequently:

| Directive | Current session | Requested fields | Pending prefix | Fresh pledge |
| --- | --- | --- | --- | --- |
| session only | discarded | retained | retained until claimed | cleared |
| parameters only | must already be init | defaults | cleared | retained |
| session and parameters | discarded | defaults | cleared | cleared |

The parameter-only pledge distinction is structural: the pledge is not inside
the requested parameter record. Existing Rust `Encoder::reset_parameters`
and `CompiledStreamEncoder::reset_parameters` deliberately clear their pledge
with frame-option defaults. A request-backed counterpart must document this
same Rust convention explicitly if retained; it must not claim exact native
parameter-only pledge semantics.

Rust streams remain in Finished until explicit reset; native frame completion
returns the session to init. Therefore parameter reset can be accepted after
a Rust completed frame while preserving completion and counters, but another
frame still requires session reset. Failed and active Rust sessions must reject
parameter-only reset without recovery; reset-all is the recovery operation.

## Existing implementation boundaries

Module 40 keeps the request in the public wrapper and an optional copy in its
private shared engine. Constructors establish equality. The compiled owner
uses None and retains its fixed profile. Request replacement/reset must update
both request copies together without touching compiled-owner behavior, or
introduce a single source of truth whose invariant is equally explicit.

The initialized profile is a historical observation of the current/completed
frame. Replacing requested settings at a clean boundary must not pretend that
a frame is already initialized. Parameter-only reset after Finished must not
rewrite the profile of the completed bytes. Session reset clears the
initialized observation; the next start resolves anew from requested settings,
pledge and pending prefix content length.

## Upstream test relationships and next qualification

`tests/fuzzer.c:3091-3118` contrasts dictionary persistence across session
reset with dictionary removal on resetting parameters; its setup includes
loaded dictionaries and is not fully ported by prefix-only coverage.
`tests/zstreamtest.c:2379,2431,3075` applies bulk parameter records in static
or streaming contexts. Static workspace behavior and workers remain separate
open obligations. The next bounded tests should directly cover bulk request
replacement, frame-option replacement, all three reset scopes, rejected active
stage changes, prefix removal/preservation, admission failure before publication,
allocator non-allocation at reset and subsequent frame reconstruction.

## Executable analysis observations

An external helper runs nine reset combinations (clean/active/completed times
session/parameters/all) and three bulk-replacement stages against the same
pinned revision. Normal and ASan/UBSan builds agree on all twelve rows. Clean
parameter reset restores level three and checksum off, clears the pending
prefix, and leaves the internal pledge-plus-one at eight for a seven-byte
pledge. Session reset clears that pledge but retains an unclaimed prefix.
Active parameter-only reset and active bulk replacement return stage-wrong
without changing requested fields. Clean bulk replacement keeps the prefix.
Completed native frames already have init stage and an unknown next pledge.

The probe uses internal fields only to observe pledge and pending-prefix
ownership. It is analysis evidence, not an implemented Rust lifecycle API or
a completed official test port. Sanitizer leak detection was disabled. Raw
helper, commands and rows are in
`evidence/2026-11-05-reference-streaming-parameters-analysis.tar.gz` with the
matching JSON receipt.
