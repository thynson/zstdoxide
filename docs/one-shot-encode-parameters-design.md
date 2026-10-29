# Parameterized dictionary-backed one-shot compression

Module 36 follows reusable one-shot compression dictionary and prefix selection.
This design and work decomposition precede implementation; promote
implementation only after Module 35 closes. The bounded scope is combining the
already-qualified reference parameter profiles with immutable dictionary
selection in caller-owned one-shot output.

## Ownership and upstream counterparts

The pinned facebook/zstd v1.5.7 revision
`f8745da6ff1ad1e7bab384bd1f9d742439278e99` already has both halves in Rust:
`inspect_compression_level`, `CompressionParameterRequest::resolve` and the
shared adjust step port `ZSTD_getCParams_internal`,
`ZSTD_getCParamsFromCCtxParams` and `ZSTD_adjustCParams_internal`, while
`compress_at_level`/`compress_with_parameters` execute the bounded Fast,
DoubleFast, Greedy, Lazy, Lazy2, BinaryTreeLazy2 and Optimal/Ultra/Ultra2
profiles for dictionary-free input, and modules 33–35 deliver dictionary
frames, caller-owned output and call-scoped selection on the ordinary Greedy
path. Upstream combines these in `ZSTD_compress_advanced_internal` and
`ZSTD_compress2`: one resolved parameter record and one referenced CDict seed
the same compression attempt, `ZSTD_compressBegin_internal` loads dictionary
indices into the selected strategy storage under the resolved window, and
`ZSTD_checkDictValidity` expires dictionary content beyond that window. Native
CCtx parameter lifecycle, reset directives, LDM, workers and session semantics
remain out of scope; do not infer identical workspace reuse or error
precedence.

The combined emitter owns no new state between calls: a resolved record (plain copied
scalars) and an immutable `DictionaryView` enter each call exactly like the
stateless dictionary view does today. Inside one call, the in-window suffix of
dictionary content is staged contiguously before the input in per-call
scratch, and the existing profile parsers run over that combined borrow
unchanged: match extension crosses the dictionary boundary contiguously, the
existing candidate floor provides exactly `ZSTD_checkDictValidity`'s window
expiry, and combined-space positions produce decoder-correct offsets by
construction. This preceding-content staging matches the raw-dictionary
refPrefix semantics the decoder already applies; it avoids forking every
profile parser into a dictionary-aware variant and copies at most the resolved
window of dictionary bytes instead of seeding each storage family. It is
per-call scratch released before return, not retained workspace, and it
replaces the earlier seed-based wording of this design for that reason.
Frame flags follow the existing parameterized-profile family (known content
size, no checksum) plus the dictionary ID field the dictionary paths already
emit; the ordinary checksummed Greedy bytes of modules 33–35 remain the
default-path contract and are not redefined. Caller-owned destination,
one-block staging, independent scratch domains, attempt-exact prefix
consumption and persistent selection all keep their existing contracts; the
coordinator stores no options and no workspace.

## Proposed public contract

Provide parameterized variants of dictionary-backed one-shot compression into
caller-owned output: resolve a `CompressionParameterRequest` against the
actual source size and the dictionary's size, then compress with an optional
immutable view and the resolved record through ordinary, dictionary and
coordinator entrypoints. Resolution errors surface before output admission or
allocator work. The existing dictionary and parameterized APIs keep their
exact current bytes, flags and bounds.

State the supported strategy subset for dictionary seeds explicitly and
reject the rest through the existing parameter error; do not silently
substitute a strategy or drop the dictionary. Resolved window bounds must
expire dictionary content exactly as `ZSTD_checkDictValidity` does for the
supported profiles, and must respect each profile's existing window and table
caps; a dictionary larger than the resolved window contributes only its
in-window suffix semantics, matching upstream rather than inventing a local
rule. min_match and target_length thread to the selected parser unchanged.
Prepared entropy reuse follows the existing dictionary contract for emitted
blocks only. No streaming, live context reset, LDM, workers, checksum
toggles or arbitrary frame settings are added.

## Original assertion boundaries

Pinned tests/fuzzer.c level-varying groups mix levels with LDM, workers and
deterministic prefix storage; only the level-with-dictionary axis without LDM
is in scope. The prefix determinism group's adjacent-versus-independent source
storage question interacts with dictionary window expiry; consult the existing
manifest status and preserve the exact preparation before claiming any newly
ported original assertion. A synthetic level-and-dictionary sweep over the
bounded profiles is a supplement even when it covers the same numeric levels.

## Qualification before promotion

Check every supported strategy and boundary level (minimum, default, maximum)
with empty, raw and prepared dictionaries: exact frame bytes against the
corresponding dictionary-free parameterized path where the dictionary is
empty, unchanged dictionary bytes, guards/suffixes, attempt-exact consumption
and persistent survival, dictionary window expiry at the resolved boundary,
and independent scratch domains under allocator failure. Compare guarded
native schedules resolved from the same levels with the same dictionaries
through the pinned C library without claiming native compressed-byte
identity, capacity precedence or workspace equivalence. Qualify under concrete
no-atomics ARM code generation, running foreign Miri only if this module adds
or modifies unsafe code, then the complete feature/release matrix, then timing
with identical stateless/coordinator workloads and explicit setup placement
before closing the module.
