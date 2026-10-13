# Ordinary Lazy-family frame protocol, version 1

This specification follows the checked direct Greedy/Lazy/Lazy2 parsers. It
freezes source-only ordinary input hypotheses before running a new C helper.
Use the unchanged external facebook/zstd v1.5.7 reference at revision
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`. No ordinary Lazy capture is claimed
by this specification, and no Rust result defines an expected C result.

## Scope and inherited contracts

The separately versioned helper `scripts/oracles/lazy_frame_v1.c` will preserve
all ownership, source bounds, publication and observation contracts of the
immutable `greedy_frame_v3.c` (SHA256
`0e7505aa18263f1483320fb697d95ea616daa9b471a217c3445ef154d50598c1`).
The reference review and complete inherited contract are in
`reference-greedy-frame-oracle.md`, with its initialization correction in
`reference-greedy-frame-helper-v3-review.md`. Earlier helpers and captures remain
unchanged. This new protocol differs only in its explicit depth/strategy selector
and the metadata identifying that extension.

Admit depth 0, 1 or 2, mapped to ZSTD_greedy, ZSTD_lazy or ZSTD_lazy2 respectively.
All three strategies use the same automatic row threshold (W > 14), as reviewed
in `zstd_compress.c:225-262`, and the same no-dictionary shared parser. Retain
resolved automatic selection from `ZSTD_CCtxParams_init_advanced`; do not reset
it to auto before internal begin. Observe the applied tuple, including strategy,
and require exact equality with the requested tuple on every fresh context.

Continue to bound W10..21, H6..19, M3..7, T0..131072. Chain C6..18/S1..8 and row
C6..29/S1..29 retain their independent resource contracts. The row branch must
not read or allocate an inactive chain owner. Clamped keys are 4..6 bytes; row
searches keep the sixteen-byte logical tail, chain searches eight bytes. Lazy
speculation includes the tail equality after a checked increment. Guarded source
leading/suffix storage remains 64/262656 initialized bytes, accommodating all
physical reads and cache work without changing logical source membership.

All other capacities remain fixed: source 4 MiB, block 128 KiB, 64 scheduled
calls, 2 MiB scratch, exact bounded accumulated frame and separate replay,
before-table and optional whole-frame owners. Review each allocation and actual
read/write extent in the completed helper before execution. No dictionary,
worker, LDM, splitter, external producer or collector is enabled.

## Commands and observations

The helper accepts these canonical commands (DEPTH is the final argument):

- `metadata`: emit schema version 1, protocol `lazy-family-ordinary-v1`, depths
  `[0,1,2]`, strategies `[3,4,5]`, pinned revision, native widths and all capacities.
- `frame CASE INPUT SCHEDULE OUTPUT WHOLE_OR_DASH W H C S M T DEPTH`: initialize
  one fresh context, make the exact scheduled Continue calls and empty End,
  decode the complete frame, and optionally compare aligned whole compression.
- `header PLEDGE W DEPTH`: observe a header-only zero Continue. Mark this result
  incomplete and undecoded, even when its bytes agree across depths.

Reject wrong counts, noncanonical/overflowing selectors and depths outside 0..2
before initializing a context or creating an output. Case names begin with the
matching `d0_`, `d1_` or `d2_` prefix. The begin record already carries the full
applied parameter tuple; all record shapes apart from metadata remain identical
to the frozen Greedy protocol. The independent validator must compare that tuple
to the depth-tagged recipe without rewriting a strategy to pass an older check.

Retain every chronological candidate, committed/pending owner, repeat/entropy
state, block decision, complete table fingerprint/delta, raw nextToUpdate,
skipping, salt/entropy and C cache record. Even rejected speculative matches
change sampling; raw/RLE decisions retain those writes while discarding candidate
coding state. Tiny calls and zero calls retain their inherited freshness and
window-floor rules. Final deltas do not claim an intermediate insertion trace.

## Frozen input families

Start with all 138 immutable ordinary Greedy and tiny-floor recipes at every
depth (414 cases). Source bytes, schedules, scalar parameters and whole-compare
flags remain identical; only names and the explicit strategy change. Preserve
all 138 depth-zero complete observations and frames as exact predecessor
controls, allowing only the documented case-name prefix in record comparison.
Retain all 15 header boundary requests at each depth (45 partial observations).

Add the 16 pre-frozen constructed score sources for each chain/row family at
each depth (96 cases). Run their 4096-byte prefix as an ordinary first block,
then the 96-byte body, with W14 for chains and W15 for rows. These cases inherit
only source bytes, not synthetic table seeds or repeat offsets. They are
hypotheses about naturally sampled histories; no score witness, winner, frame
kind or improvement is promised. Keep every non-witness. Total planned ordinary
cases: 510, including 39 aligned whole-frame comparisons.

The source-only manifest records full source SHA256, schedule, all seven
parameters, explicit depth, selected branch, predecessor identity when present,
and hypothesis. Its generator and predecessor dependencies are pinned before
capture. Validate integer types independently of booleans, exact depth/name
agreement, all source/schedule bounds and duplicate identities in normal and
optimized Python without executing a codec or child process.

## Execution gates and evidence

Before first execution: commit this specification and immutable inputs; implement
and review the helper with strict normal/sanitizer syntax checks; implement the
independent record validator and malformed controls; then implement and preflight
a bounded capture controller and offline auditor. Compile all 26 upstream C
units separately for normal and ASan/UBSan builds. Preserve compiler, dependency,
source, binary and command identities and exact stdout/stderr. Require complete
normal/instrumented observation and frame equality and empty success stderr.

Independent validation replays all sequences, publications and complete maps,
checks depth-zero predecessors, guards all byte extents, and rejects coherently
modified build pairs in offline controls. Preserve failed captures unchanged;
version a corrected helper/controller rather than changing frozen evidence.
Projection into Rust ordinary trajectory fixtures follows successful capture and
auditing. Public strategy admission, public interoperability and performance
remain later independent gates.
