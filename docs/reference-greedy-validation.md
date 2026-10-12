# Bounded reference Greedy validation in progress

Module 14 is in progress. Source design, pre-Greedy regression baselines and the
tiny and broader direct C oracle checkpoints are complete. Shared Rust hash
values, reference-chain and row storage backends are implemented. Greedy
parsing, ordinary-frame state, public Greedy admission and broader test migration
remain in progress. The existing independent named Greedy policy is not
an implementation of this reference strategy.

## Preserved behavior before implementation

Sixteen clock-free DoubleFast/Fast cases preserve twelve distinct complete
frames (5,680,125 bytes) and twelve allocator survey fields per case. Five new
regression tests pass on the safe default, each of twelve unsafe features alone
and all features together. The four original Fast baseline tests also pass on
safe/all. Strict Clippy for the new target, safe/all no_std checks, formatting
and unchanged Fast/named offline baselines pass. Exact allocator bytes are
qualified to the pinned aarch64-apple-darwin compiler/target; portable checks
retain complete frames, owner lifetimes and release invariants.

The [baseline evidence](evidence/2026-10-12-reference-double-fast-baseline.json)
and its source fixture retain the original clock-free capture identity. Both
normal Python and `-O` rejected fifteen baseline mutations with process creation
disabled. These tests establish unchanged prior behavior, not new Greedy support.

## First direct C checkpoint, 2026-10-12

The [reviewed adapter](reference-greedy-adapter-review.md) implements the
[bounded packet contract](reference-greedy-oracle.md). Thirty-six recipes were
frozen before execution: seventeen chain cases, seventeen corresponding row
cases and two additional salted row cases. Fresh and prefixed tail boundaries,
periodic input, minMatch dispatch and unavailable repeats are covered. Each
recipe runs with two source-padding patterns in both C builds, yielding 144
complete sequence/table observations. All direct outputs are byte-identical
between normal and ASan/UBSan builds.

Two separately allocated fresh contexts per build use distinct nonzero payload
fills. All four contexts observe initial salt/entropy zero, then fresh row salt
`0x835892b4bb2cae74`, entropy zero and zeroed tables after initialization. The
applied tuple is W15/C6/H6/S1/M4/T0/Greedy with row mode explicitly enabled.
The independent bitmix calculation agrees. Context reuse and dictionary salt
contracts are not established by these observations.

All 26 linked C translation units plus the helper are built from the pinned
v1.5.7 sources in each mode. No support archive is linked. ASan/UBSan instruments
the helper and every linked unit, with LeakSanitizer disabled and upstream
per-function pointer-overflow exclusions retained. This is not a claim that
sanitizers check every upstream expression. Compiler is Apple clang 17.0.0,
target arm64-apple-darwin24.6.0; full arguments, dependency closure, executable
and object identities are preserved. Thirty-seven malformed packets reject
before parser execution per build, for 74 rejections. The qualified campaign
contains 210 recorded commands.

The first full campaign passed all per-case checks but failed its final generic
cross-build equality. Only the opaque context's guarded allocation peak differed:
149,198 bytes normally and 151,502 under ASan, with two allocations and complete
release in both modes. Pinned `zstd_cwksp.h` adds internal ASan redzones; direct
arena allocations and results were unchanged. The successor controller excludes
only this mode-dependent memory field from cross-build fresh-salt equality,
retains both exact values, and still compares every other salt/state/owner field.
The helper, input recipes and per-record validator did not change. The original
failed report, raw logs, input packets and exact first controller source remain
archived; it has not been relabeled as a successful campaign.

The independent offline verifier replays all 210 command records, every compile
and link argument, source/dependency identities, raw input packets, completion
markers, full sequence replay and complete before/after maps. It compares live
observations against a separately pinned immutable fixture, so coherently
rewriting a log and its local hash cannot refresh expectations. Normal Python
and `-O` each pass two positive controls and reject thirty forged captures with
child-process creation disabled. The negative controls include missing compiled
units, altered sanitizer flags, archive linkage, changed fields/maps/salt,
changed packets and the qualified context memory values.

The permanent fixture contains 147,120 NDJSON bytes in 38 complete groups:
36 direct cases plus normal and instrumented fresh-context observations. Its
capture manifest SHA256 is
`18e0ff3d695627c20ef159ecda1ee027ce535d98a55e0b84c261a0ef34c0ffa4`;
the observation pack is
`a2bac02eeeb65292b92892605d15b043097ed94651b4be494fec0427b73b6fff`.
Normal checks consume it without invoking C or importing a codec.

The [capture report](evidence/2026-10-12-greedy-tiny.json) records the evidence
archive: 1,079 regular members, 7,648,534 uncompressed bytes, 1,140,444 archive
bytes, SHA256
`26b361a9b10bec58fd1f74d58eaa1146359a63c8b01683de07393e01fd918ccc`.
It retains both campaigns, all logs/packets, 63 used C source/header files,
local source closures, controls and complete observations. Compiled objects and
executables are omitted with their identities retained. Every archived member
was reread and compared to its original bytes.

## Broader direct C checkpoint, 2026-10-12

The 137 pre-frozen recipes (63 chain and 74 row) run with two padding patterns
under both builds: 548 direct observations, all byte-identical across builds.
Fifty-three malformed packets per build produce 106 checked rejections. The
four fresh contexts retain the same salt and qualified memory peaks as the tiny
capture. All 26 linked C units and the helper are rebuilt in both modes; all
444 command records, dependency/source identities and raw transports are retained.
The unchanged helper's sanitizer qualifications above still apply.

Observed consequences include the following; the archived coverage check derives
them from immutable C records, without implementing the Greedy parser:

- W10 accepts distances 1023/1024 and rejects 1025. Both branches accept a head
  at distance 2,097,152 for a 64-byte match, even beyond chain ring capacity.
- Distinct colliding four-byte keys emit no match with C6, but a 32-byte match
  at distance 128 with C7. Equal-length candidates select the most recent
  position across the supplied chain budgets and all three row widths.
- Row initial heads wrap through their 15/31/63 usable cells, preserving cell
  zero. At direct pending gap 384, 385 positions change; at gap 385, 129 change.
  This does not establish the separate ordinary wrapper's adjustment.
- Period-four sources bracket the stored long-ML flag: expanded match length
  65538 has no flag and 65539 does. The three long-literal hypotheses all produce
  the long-LL flag at literal lengths 65760, 65795 and 65796; they do not bracket
  the exact LL cutoff.
- Both branches retain a skipping flag after 4096 noise bytes and clear it on
  the later periodic match. Row salt entropy wraps from initial `0xfffffff0`
  to 27,664,900, exactly sixteen below the zero-initialized control's final value.

These are complete before/after maps and sequence records, not chronological
probe tracing. The fixed pack is 5,104,709 bytes in 139 groups, SHA256
`0366364cb7a1123cf0e9580770037a50df5c23bcba89a4c47fd1bed8bc3c0d27`;
the capture manifest is
`f61e92df7e1b58d548b7058d30cbfb7228f3cc1e4cfd3fbad9b3050e0cc20e41`.
The independent offline checker compares every raw observation with this pinned
pack as well as validating maps, sequence replay, compile/link commands and
source closure. Coherently rewriting a log and its local checksum cannot update
expectations. Python and `-O` controls each verify two original/restored positive
captures and reject thirty forged captures with child-process creation disabled.
The [evidence report](evidence/2026-10-12-greedy-families.json) retains exact archive
identity, all logs/packets, 63 used C source/header files, controls and the observed
coverage audit; compiled objects/executables are omitted with hashes retained.
Archive verification compares the complete member inventory and original bytes.

## Rust scalar and storage checkpoints

Shared safe Greedy keys/row layouts now admit H6..19 without changing prior key
domains. Four tests check dimensions, exact-width tails, incompatible keys, row
geometry and salted high bits against 1,728 prior independent C primitive
vectors. Safe/all runs, strict safe/all Clippy and safe/all-unsafe no_std
compilation pass. This value-only layer owns no allocation or pointer.

The safe reference chain owns independently allocated heads and cyclic links.
Five initial tests cover full maps, raw-index bias, valid old heads, malformed
indices/links, bounded traversal, independent non-clone owners, both allocation
failure ordinals and complete moves/swaps/reset. Safe/all focused runs, strict
safe Clippy and no_std compilation pass. Parser/full-oracle integration and
performance comparison remain separate tasks.

The independently gated `unsafe-reference-chain` backend preserves those owner
contracts and checks every access at runtime before entering a documented unsafe
block. Six tests now include an unbounded source-history model of sparse cyclic
traversal, and pass on safe, isolated-feature and all-feature builds; isolated
no_std and strict feature/all Clippy pass. Safe row storage also passes six
safe/all tests covering complete position/tag maps at every row width, snapshots
across insertion, mixed tags, empty/expired/future cells, budget limits, malformed
heads, both owner extents and all allocation failure ordinals. Its strict safe
Clippy and no_std check pass. The `unsafe-row-table` backend passes the same six tests in isolation and with
all features. Runtime row/owner checks precede every locally documented unchecked
access; candidate snapshots remain safe values. Strict feature/all Clippy,
isolated-feature no_std and all fourteen unsafe features without std pass.
Full frozen parser/map integration and deterministic performance comparisons
remain pending.

The private Greedy admission plan passes six safe/all tests plus strict safe
Clippy and no_std compilation. Five previously frozen C Greedy query tuples
select one chain and four row plans. Tests cover actual known-source adjustment,
branch-specific resource caps, independent target payloads and complete output
bounds, unsupported-strategy precedence, empty hints and nontruncated large
source scalars. This does not expose Greedy in the public request API.

The borrowed block/cursor layer passes five safe/all tests plus strict safe
Clippy and no_std compilation. It retains distinct 8/16-byte search tails, strict
main-loop versus inclusive repeat cleanup, per-probe window floors and checked
anchor-relative skipping. The final coordinates/flags on six no-match noise
schedules agree with the frozen direct C observations. This is a scalar schedule
check, not yet a complete parser or full table-map comparison.

The direct projection now freezes all 173 tiny/broader cases into 7,985,140
little-endian bytes, SHA256
`5bc0e95cd395d8be489638b2f03ead1494172d92c7c1b0ede49c97cf3e4a0064`.
The generator checks immutable source checkers before importing them. An
independent binary reader compares every projected field and case extent with
those C observations; both Python modes pass two positives and reject 24
mutations with process creation disabled. Separate publication-policy controls
pass two positives and reject four overwrite/partial-write/symlink cases per
mode. Their byte-source substitution is explicitly qualified in the
[evidence](evidence/2026-10-12-greedy-projection.json); it does not replace source
verification. The Rust fixture reader is now checked separately below; full parsing remains pending.

The dependency-free Rust fixture adapter passes five tests on the safe default,
`unsafe-reference-chain` alone, `unsafe-row-table` alone and all features. It
replays every sequence and wire repeat, validates complete map fingerprints and
hash/tag membership, and compares all 346 seeded Rust owner maps with C. Controls
reject truncated/count/extent errors, malformed dimensions, forged sequences,
invalid owner roles and coherently rehashed invalid raw positions or reserved
heads. Strict safe/all Clippy, formatting and no_std compilation pass. These
checks establish the reader and initial storage states, not complete Rust Greedy
search/parsing or final parser-state equivalence.


The safe chain search passes four tests on safe, unsafe-reference-chain alone
and all features. Complete final heads, links and update frontiers agree with
22 frozen no-match schedules (including two final skipping states) and 14
first-probe full-block matches, including a 2-MiB head outside the chain ring.
Independent byte/argument tests cover strict tie preservation, bounded attempts,
logical tails and rejection before scalar-invalid mutations. A full-block
sequence alone does not prove a first-probe match: cases with later frontiers
and backward extension are reserved for complete parser validation. Strict
safe/all Clippy and safe/isolated-chain no_std compilation pass. Full parsing
and ordinary state retention remain separate work items.


The combined chain/row search suite passes eight tests on safe,
unsafe-row-table alone and all features. Twenty-five row no-match schedules
match complete final positions/tags, update frontiers and salt entropy,
including all six 383/384/385/575/576/577 pending gaps, two final skipping states
and wrapping entropy. Selected first-probe full-block matches also agree in
complete maps, including the 2-MiB head. Fourteen separately selected row-width,
wrap, zero-tag and capped-budget cases choose the frozen newest tie; their
remaining parser trajectories are not yet covered by this search-only check.
Invalid row arguments preserve tables and progress scalars. Strict safe/all
Clippy and safe/isolated-row no_std compilation pass. Scalar hashing intentionally
has no Rust counterpart for the preserved C prefetch cache.


The complete direct parser passes four tests on safe, unsafe-reference-chain
alone, unsafe-row-table alone and all features. All 173 frozen cases run with
both exact source slices and independently padded later bytes: sequences,
expanded/raw length fields, wire offset intents, literal bytes, long markers,
raw/wire repeat triples, next-to-update, skipping, row salt entropy and every
final table cell match C. Independent overlap replay reconstructs every source.
This covers the later-probe backward-extension cases excluded from single-probe
search tests. C prefetch-cache cells remain evidence only.

Both output-reservation failures preserve contents, table maps and progress on
both branches; four independent non-clone owner identities release correctly.
Borrowed non-clone owners support reuse with exactly four initial allocations,
no growth calls and no additional requests on the second parse. Invalid entry
arguments preserve scratch and sampling. Formatting, strict safe/all Clippy,
safe no_std and combined chain/row no_std compilation pass. The direct parser
returns candidate wire state only; ordinary retention/publication is pending.


The separate [ordinary-frame protocol](reference-greedy-frame-oracle.md) now
pins 126 source-only inputs: 55 chain and 71 row, 6,959,842 source bytes,
355 scheduled calls (347 positive), 13 aligned comparison hypotheses and
15 incomplete-header probes. Both normal Python and `-O` verify the immutable
manifest/generator, pass three metadata controls and reject 43 mutations with
process creation disabled and zero child attempts. The
[evidence](evidence/2026-10-12-greedy-frame-recipes.json) preserves the exact
checker driver and input identities. No ordinary C execution, block-kind or
repeat-discard witness is claimed; helper review and capture are next.


The new ordinary helper now has a completed
[pre-execution source/ownership review](reference-greedy-frame-helper-review.md)
and strict normal/sanitizer-configured C11 syntax checks. Its selected table
extents are checked against initialized workspace bounds before observation;
row mode never reads the inactive chain reservation. Complete owner deltas,
raw/expanded lengths, sampling metadata and candidate/publication states are
separate records. The [review evidence](evidence/2026-10-12-greedy-frame-helper.json)
pins the helper, reviewed sources, compiler and exact syntax commands. No helper
mode, ordinary frame capture or sanitizer runtime has executed at this item.

## Remaining scope

Constructed direct states do not establish ordinary context/frame trajectories
or automatic public branch selection. Ordinary retention/publication, public admission and related official assertions
remain in progress. No new official source-unit obligation is marked
complete. Final every-feature/platform/interoperability matrices and Greedy
measurements remain open, as do the complete roadmap and upstream migration.
