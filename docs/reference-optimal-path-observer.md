# Bounded Optimal path observation plan

This gate isolates optLevel0 path selection in facebook/zstd v1.5.7,
revision `f8745da6ff1ad1e7bab384bd1f9d742439278e99`. It does not establish
ordinary CCtx match selection, startup pointer behavior, retained blocks, public
admission or complete codec equivalence. Those remain separate gates.

## Observation boundary

Run the pinned `ZSTD_compressBlock_opt_generic` with its original frequency,
price, forward transition, repeat publication, backtracking and sequence-storage
code. A generated copy outside the repository replaces only the selected
`getAllMatches` function pointer with a bounded scripted callback and adds
observation calls at reviewed unique source anchors. Preserve the original file,
its digest, the exact substitution manifest and the resulting translation unit.
Every substitution must occur exactly once. No replacement parser is the C oracle.

The scripted callback returns source-valid candidates supplied by the request.
It records every query position, repeat history, literal-zero flag and minimum
length. Candidate lengths strictly increase; offset bases are explicit sequence
intents. Resolve repeats against the actual query history and reject unavailable
history or mismatching source bytes before copying any candidate. Unlisted
positions return no candidates. This intentionally separates path decisions from
the already qualified sorted match finder. It must never be described as an
ordinary compression-context capture.

The first protocol bounds source length to 9..512 bytes, at most 64 scripted
positions and eight candidates per position. Minimum-match parameter is 3..7;
the effective minimum is three for parameter three and four otherwise. Applied
target length is 0..4095; literal pricing is enabled or disabled. Initial repeat
history is the pinned fresh `[1,4,8]`. A position is at most source length minus
eight, and each candidate must end within logical source. Direct distances must
fit the available prefix; repeat validity is checked at every actual callback.
Source, script, emitted sequences and events have explicit static capacities.
This protocol does not exercise a path reaching the 4096-position horizon; that
requires an additive wider protocol before production path integration.

The oracle borrows a padded, initialized source arena but compares only logical
bytes. Guard and source equality checks surround execution. Every path cell,
frequency cell and sequence/literal workspace is initialized before invocation.
Use exact workspace sizes, independent canaries and checked output capacities.
The no-dictionary source begins at a reviewed prefix offset within the source
arena. The scripted callback never forms a pointer before the source. Its use
therefore does not settle the ordinary match finder's fresh-repeat startup gate.

## State to retain

An event stores the complete initialized path prefix of `source_length + 3`
cells, each containing price, offset intent, match length, literal length and
three repeats. The rest of the 4099-cell path owner must remain zero, checked at
every event. Retain absolute input/anchor positions, current relative position,
last reachable position, and the optional final stretch. Fields that upstream
leaves stale remain observable initialized values, not invented semantic fields.

Retain complete literal/LL/ML/OF frequency tables, sums and cached base prices at
initial rescale, each sequence update, price refresh and final return. Each match
callback records its exact ordered candidates. Each emitted sequence retains
literal bytes, offset intent, match length, and repeat history after the segment.
Final output includes trailing literal length, path owner, frequency state,
sequence list, repeat history, source equality and every guard result.

Observe these boundaries without changing their order:

1. Initial frequency rescale and the prefix-start one-byte advance.
2. Every scripted match query and its returned candidates.
3. Origin initialization and initial match-price seeding.
4. Each literal correction followed by deferred repeat publication.
5. Each later match relaxation, including early-abort and horizon growth.
6. Final stretch selection, including sufficient-length exits.
7. Each reverse-path predecessor read and destination rewrite.
8. Each stored sequence/statistics update and final price refresh.

No uninitialized local is passed to a hook. At branches without a meaningful
relative position or final stretch, use an explicit absent value. The observer
must enforce an event budget before writing an event and fail rather than
silently truncating a trace.

## Independent replay and falsification

Reuse the already qualified independent Python frequency/price model. Represent
path cells as owned integer records; record active phase separately. The replay
uses checked arithmetic and source ranges and never invokes C or production Rust.
It reproduces whole-bit optLevel0 rules, including:

- Literal extension replaces the existing cell on equal cost (`<=`).
- Match relaxation replaces an existing cell only at strictly lower cost (`<`).
- Later match lengths are visited downward, with optLevel0 stopping the current
  candidate's downward scan on the first non-improvement.
- Unpromising positions skip search when the next price is no more than the
  current price plus 128; this is not the literal tie rule.
- Initial sufficient-length exit is strict; later exit also considers source
  end and the 4096-position horizon, the latter deferred in this first protocol.
- Repeat history is published when a selected match reaches its end, rather
  than when a hypothetical match price is first installed.
- Forward cells describe a match followed by literals; reverse reconstruction
  overwrites them with literals followed by a match. Reads must precede writes
  that could erase a predecessor. Preserve the pinned implementation's exact
  assignments even where an intermediate write is immediately overwritten.
- Sequence frequency updates occur in emitted order; cached prices refresh only
  after the whole selected segment.

Before C execution, use hand-derived traces for no match, immediate match,
zero-literal repeat publication, equal-price literal replacement, strict match
replacement, skipped search, downward early abort, a path with multiple matches,
trailing literals and reverse reconstruction. Source-only recipes must actually
exercise their named branch in independent replay; a name is not coverage proof.
Reject malformed transport, unavailable offsets, non-increasing candidates,
source overruns, negative/overflowing costs, unreachable predecessors and cycles.

After capture, validate exact canonical records in normal and optimized Python.
Forge candidate order, repeat state, equal-price decisions, predecessor links,
phase tags, path cell fields, frequency increments, event counts and completion
records. Offline validators must spawn no child process. Preserve failures and
corrected additive evidence instead of rewriting frozen captures.

## Transport checks

`oracles/optimal_paths.py` defines the immutable binary request and independently
checked repeat-distance/publication algebra. Normal and optimized Python each
round-trip four positive requests and reject 38 malformed requests, packets and
unavailable-repeat examples. These source-only checks spawn no child process.
The request parser checks byte extents before decoding candidate arrays; dynamic
repeat candidates still require independent replay before C execution. Evidence:
`evidence/2026-10-14-optimal-path-protocol.json`.

## Independent replay preflight

Seventeen frozen requests cover all seventeen named transition branches before
C execution: 1,302 events, 824 match queries and 28 emitted sequences per replay
set. Hand-derived cases verify immediate selection, all-literal fallback, deferred
repeat history, explicit repeat publication and strict sufficient-length equality.
Normal and optimized Python each reject 30 forged complete traces plus unavailable
dynamic history and event-budget overflow. Both runs spawn no children. Full
512-byte no-match input and all minimum parameters are included; the 4096 horizon
remains intentionally unqualified. Evidence: `evidence/2026-10-14-optimal-path-model.json`.
These are independent model controls, not an upstream comparison result.
