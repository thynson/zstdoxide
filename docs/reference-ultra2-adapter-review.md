# Ultra2 native adapter review before execution

Pinned upstream: facebook/zstd v1.5.7, revision
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`. The source-only lifecycle model,
recipe manifest and `reference-ultra2-protocol.md` determine expected behavior.
The observer must pass pure preflight before any native execution. This document
records the source review; it is not a claim that capture has already passed.

## Original algorithm and read-only instrumentation

The ordinary Ultra observer contributes fifteen previously reviewed hooks. Four
additional hooks observe the wrapper entry, seeding phase start, pre-rescale pass
entry and post-reset coordinate state. Every anchor is unique in the pinned
original source. Removal in reverse order must recover the original byte stream,
including the complete five-predicate gate, tmpRep copy, original first-pass
call, ZSTD_resetSeqStore and backwards-base/history-frontier arithmetic. Neither
match-provider selection nor any match, path, price or cache decision changes.

The hooks update only observer-owned counters and snapshots. No hook changes
CCtx state, source bytes, frequencies, paths, search tables, repeat arrays or
sequence data. The observer checks that the seeding pass receives a distinct
repeat-array owner and the encoding pass receives the original caller owner;
both begin with the original values. At native reset, every physical hash/tree/
short-key cell must equal its final seeding snapshot. Frequency state is emitted
before/after reset and independently compared, not assumed to be freshly empty.

## Source, coordinates and owner boundaries

A real zero-initialized, guarded CCtx and workspace own all algorithm tables and
token buffers. The adapter uses standard parameter setup, disables LDM/workers/
post-splitting and verifies the actual applied geometry. The source allocation
includes SOURCE_CAP+64 leading guard bytes, so even the largest backwards-base
movement remains inside the same C allocation. The initial native bias is two.
The caller sets fresh source coordinates before the one direct wrapper call;
no entropy encoder or frame publication follows it. The adapter also reproduces
the two `ZSTD_buildSeqStore` setup assignments: `symbolCosts` borrows the fresh
previous block entropy owner (with HUF repeat mode none), and literal compression
mode comes from the applied context parameters. Both are checked throughout;
without these assignments a direct wrapper call would not satisfy the parser
contract, especially for disabled literals.

For threshold requests below 64 bytes only, the workspace allocation hint is
64; actual parser source length stays 7/8/9. This keeps valid ordinary table
geometry without claiming public small-input strategy admission. Seed mode zero
places one zero-literal sentinel sequence into the initial store. All guards and
sequence checks account for that opaque prefix, whose fields must remain intact.
The reported sequences and copied literals cover only newly appended parser
output. This is a private lifecycle oracle, not a frame compression surrogate.

Each padding run has fresh allocation ownership and zero observer baselines.
All source prefix/tail guards and every allocation guard are checked throughout.
Before reset, the observer records discarded sequence/literal counts, tail and
candidate repeats. After reset, logical output is empty, original caller repeats
remain intact and the second invocation restarts event/phase-local counters.
Persistent sparse baselines retain old physical state across the pass boundary.

## Transport and command boundary

The descriptor reader uses O_NOFOLLOW/O_NONBLOCK, fstat and a regular-file check;
it validates bounded length and exact EOF before interpreting fields. All source
and geometry checks precede CCtx allocation and stdout metadata. Negative packets,
missing paths, symlinks, directories and FIFOs must exit with the exact reviewed
diagnostic and create no artifacts. Each positive request produces two bounded
allocation ledgers and one complete two-padding NDJSON transcript.

The command plan contains explicit dependency and effective-predefine checks,
then compiles every used codec translation unit separately for both normal and
ASan/UBSan builds. The helper is compiled with -Wall/-Wextra/-Werror. Sanitizer
flags apply to all 26 relevant translation units and original workspace poisoning
stays enabled. The embedded observed zstd_opt.c replaces its separate compilation;
no duplicate codec object is linked. The actual compiler, pinned headers/source,
generated source, full commands, environment, outputs and executable hashes are
retained. The oracle is never a Rust implementation dependency.

Independent replay checks exact source/request/generated-file inventories,
command order and exits, compiler/source identity, canonical duplicate-free JSON,
complete pass/event counts, all predicted intermediate states, normal/sanitized
transcript equality and padding allocation equality. Sanitized workspace growth
must equal the existing fourteen 128-byte redzone pairs. A capture discrepancy
must retain the failed evidence; it must not be hidden by rewriting inputs or
pretending an unexecuted check passed.

The reviewed inventory has 220 commands: four reference/compiler probes, then
for each build two dependency/predefine probes, 26 compilations, one link,
53 complete recipes, 22 malformed packets and four filesystem controls. The
helper command receives one packet and two ledger destinations. No upstream
executable is called by the independent Python models or pure preflight.

## Sealed complete-state predictions

Before native execution, a separate source-only campaign computes every canonical
wire record in full, including both padding runs, and records exact byte/line/
event bounds plus a SHA-256 over newline-delimited canonical JSON. The complete
prediction manifest is then sealed by a literal digest in the reviewed capture
specification. Its model-source closure is checked against retained snapshots.
The auditor strictly parses every native line, rejects duplicate keys/nonfinite
numbers, canonicalizes every complete record and verifies the entire predicted
transcript digest and counts. This establishes complete-state equality against
the independently executed model; it does not infer intermediate correctness
from final tokens or frames. It avoids recalculating the unchanged model for
every copied-evidence forgery. A changed transcript gets a different cache key
and must pass full canonical equality; a coherently changed model manifest is
rejected by its pre-execution digest anchor.
