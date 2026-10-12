# Bounded reference Greedy validation in progress

Module 14 is in progress. Direct and ordinary Greedy parsing, retained state
and public bounded Greedy admission are implemented and checked below. Broader
public C interoperability, platform/full-feature qualification and measurement
remain open. The existing independent named Greedy policy retains its behavior.
The sections below record successive checkpoints and their historical scope.

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


Further source review before execution found the ordinary frame loop's window
floor clamp precedes even a tiny candidate bypass. A
[versioned successor review](reference-greedy-frame-helper-v2-review.md) corrects
that assertion while retaining the original helper and evidence unexecuted.
V2 permits only that tiny frontier clamp, preserves both maps and other sampling
fields, and uses fieldwise equality without C struct-padding assumptions. Strict
normal/sanitizer-configured syntax checks pass; the
[evidence](evidence/2026-10-12-greedy-frame-helper-v2.json) records both source
identities. This is a source correction, not a C runtime finding.


Twelve additive source-only floor recipes now place lengths1..6 after two
uniform window-sized blocks, then resume fresh search: six chain W10 and six
row W15 cases, 405,942 source bytes and 48 positive calls. Original input hashes
remain unchanged. Normal Python and `-O` each reject eight mutations with zero
child-process attempts; the [evidence](evidence/2026-10-12-greedy-frame-floor-recipes.json)
retains identities and the blocked-process driver. Combined future ordinary
capture comprises 138 cases and 403 scheduled calls (395 positive), with no
ordinary runtime observations yet. The unchanged complete DoubleFast frame/owner
baseline also passes all five tests after the new private Greedy parser work.


The ordinary offline validator now checks exact record shapes/types, selected
owner geometry/fingerprints and key membership, sampling continuity, ordered
window-floor/long-gap bounds, raw/expanded sequence lengths and long markers,
independent overlap replay and compressed-only publication. Zero/tiny records
are processed in chronological order; tiny frontier clamping remains distinct
from parser updates. C cache fields are bounded retained evidence rather than
an inferred intermediate trace, and compressed payload decoding still belongs
to subsequent C/Rust execution.

Normal Python and `-O` each pass 5,184 frozen/algebraically salted primitive
checks, six explicitly synthetic structural case checks and one metadata check,
then reject 81 mutations with process creation disabled and zero child attempts.
The [evidence](evidence/2026-10-12-greedy-frame-validator.json) preserves exact
sources and the independent blocked-process driver. These synthetic records
are not stored as oracle expectations and establish no observed ordinary C
behavior. Capture-controller review and actual normal/instrumented capture remain
pending.

## Remaining scope

Constructed direct states do not establish ordinary context/frame trajectories
or automatic public branch selection. Ordinary retention/publication, public admission and related official assertions
remain in progress. No new official source-unit obligation is marked
complete. Final every-feature/platform/interoperability matrices and Greedy
measurements remain open, as do the complete roadmap and upstream migration.

## Public bounded Greedy admission

The public one-shot request/level APIs now admit reference Greedy after resolving
the actual known source once. The separately committed v3 mappings retain every
historical input, scalar observation and old result. Greedy admits window logs
through 21 and hash logs through 19. Resolved windows through 14 select chains
with chain/search caps 18/8; larger windows select rows with at most 64 visits and
no allocation or resource cap for the unused chain dimension. Unsupported
strategies still reject before resource admission, allocation or allocator clones.

Level 4 now executes every source tier: Greedy through 16 KiB and above 128 KiB
through 256 KiB, DoubleFast elsewhere. Existing Fast/DoubleFast limits and all
named encoder behavior remain unchanged. Public docs describe these independent
entry points, source adjustment, key-width clamps, target behavior, ownership
and independent entropy output. Integration-only dead-code allowances have been
removed; test inspection helpers are explicitly test-only.

Four public Greedy tests cover known empty/tiny and tier-boundary sources,
conflicting hints without request mutation, both key-width equivalences, ignored
targets, row chain-size independence and capped search, supported maximum
dimensions, pre-allocation error precedence, every observed public allocation
failure on both families, and results retaining borrowed non-clone allocators
after source destruction. They pass in safe, isolated chain, isolated row and
all-feature builds. The 42 combined reference public/baseline/original-selector
tests pass in safe/all builds; nine of these retain complete frozen Fast and
DoubleFast frames and qualified allocator observations. Five static dispatch
tests also pass in both builds. All five Rust documentation tests, strict safe/all
Clippy, formatting and safe/combined-leaf no_std compilation pass.

Private retained-state and complete-frame evidence is described in
[ordinary frame validation](reference-greedy-frame-validation.md). Public
Greedy frames still require the separately planned broader bidirectional C
campaign. Full feature/platform matrices, measurements and official migration
obligations remain open; this is not Module 14 completion or upstream parity.

## Original selector and numeric-loop assertions

The live adaptations retain all 81 `simple_round_trip.c` selector inputs and
the exact empty-source levels -1..22 and initial-match levels 1..19 from
`fuzzer.c:1554-1566` and `fuzzer.c:1637-1664`. Version 3 adds only selector
`unsupported-57` and level 4 in each loop. The selected source prefixes,
capacity-reduction budget, original input hashes, preset identity and advanced
branch's unconsumed bytes remain unchanged. Version 2 provenance tests are
retained alongside the two new successor-identity tests.

The live selector campaign now checks 62 deterministic admitted round trips,
15 unsupported results and four advanced branches; loops admit 6/24 and 4/19
profiles. Repeated complete frame bytes, original C size budgets and exact
decoded bytes are checked without claiming reused-context or caller-buffer
equivalence. All 13 tests pass in safe/all builds; strict Clippy passes. The
structural inventory still has 188 unfinished source units (149 blocked,
34 partial, five planned). No source-unit status is promoted by these bounded
one-shot assertions, and no new C execution is claimed in this item.

## Broader public C interoperability

The separately reviewed successor campaign completes all 422 frozen input/request
pairs: 372 admitted, 37 unsupported and 13 resource errors. It retains both
historical 92/159 sets with unchanged sources, requests and all 251 Rust-resolved
tuples, while positively executing their nine newly admitted records. Added
ordinary source inputs use public resolution and a single known-size call;
their older forced internal branch and schedules remain separate observations.
The small `row_search29` and `row_unused_chain29` inputs correctly become
resource-rejected chain requests after adjustment.

All 4,786 bounded commands pass. Normal/O2 and ASan+UBSan/O1 compile each of the
26 used C translation units and the helper, without a support archive, assembly,
legacy codecs or workers. Upstream sanitizer annotations are retained and
LeakSanitizer is disabled. Each of the four builds rejects all 39 malformed
packets, including old-protocol magic, with exit 2, no response and no sanitizer
diagnostic. Safe/all Rust statuses, tuples and complete frames agree; C frames
agree between normal and sanitized builds. All 25 Rust equivalence groups pass.

C decodes 1,488 Rust frames and Rust decodes 1,488 C frames, reproducing a total
of 413,994,400 source bytes. These counts include the two directions and four
build combinations, not distinct source inputs. No identical C/Rust entropy
bytes, C applied-parameter view, reused-context behavior or performance result
is claimed.

Independent offline replay checks exact commands, environment, source and
dependency closure, effective controls, captured binaries, every request and
response, scalar admission, historical tuples and decoded bytes. Normal and
optimized Python each reject 37 provenance or actual-artifact forgeries with
child creation blocked and zero attempts. Controls include removed sanitizer
flags, unexpected libraries, altered binaries, a coherently changed historical
target field, changed requests/decoded bytes and injected sanitizer diagnostics.
Frozen regenerated inputs are reused within the mutation loops; artifact checks
are still performed. The unchanged original capture passes before and afterward.

The [evidence](evidence/2026-10-12-greedy-public-interop.json) and
[archive](evidence/2026-10-12-greedy-public-interop.tar.gz) preserve every one of
14,414 capture files, including objects, executables and Rust build outputs:
1,305,890,757 original bytes represented by a 35,003,394-byte archive with 4-KiB
chunk deduplication. Sequential verification checks every archive member and
every reconstructed file against the original capture without running archived
code. Downloaded upstream sources remain outside the repository. The earlier
capture tools and evidence remain immutable. Platform/full-feature checks,
benchmarks and complete upstream migration remain separate open obligations.

## Focused platform validation

All 44 selected strict-provenance Miri executions pass: ten chain/row storage,
parser, poisoning, ownership and small complete-frame tests plus one public
tiny-frame test, in safe/all builds on both i686 and s390x. Exact nonempty test
names and counts are checked for every run. The cases cover cyclic/head bounds,
future and truncated state, failed reservations, independently owned tables,
poisoning after mutation, both effective key-width clamps and output that retains
a borrowed non-clone allocator after the source is gone. A native smoke of the
public test also passes.

The thumbv6m checks compile the no_std library and generate a caller rlib for
safe/all builds with all fourteen unsafe flags explicit and std disabled. The
caller exercises public level 4, explicit Greedy and borrowed Global allocation;
an Rc type check also retains the existing no-pointer-atomics dictionary fallback.
These are compilation/code-generation checks, not firmware linking, execution,
memory-fit or stable Allocator API guarantees. The i686/s390x results are Miri
interpretation, not native or QEMU execution.

The [platform evidence](evidence/2026-10-12-greedy-platform.json) and
[archive](evidence/2026-10-12-greedy-platform.tar.gz) preserve all 19 command logs,
compiler/cached-sysroot identities, exact selectors, test overlays, caller source
and both originally recorded rlibs. The canonical caller output is overwritten
by the second build; the safe/all artifacts were recovered from their distinct
Cargo paths by exact hashes without rebuilding. All tested production inputs
and both added tests match the immutable snapshot and delivered files. Every
one of the archive's 34 members was verified. Full feature checks, measurements
and complete upstream migration remain separate obligations.

## Completed immutable feature matrix

The immutable snapshot completes all 88 commands. Safe, each of fourteen unsafe
features, all features in debug and all features in release each pass 1,098 tests
with 54 documented ignored tests: 17 full suites, 18,666 passes, 918 ignored and
zero failures. Sixteen no_std configurations compile. Formatting and strict
safe/all Clippy pass, allocator support passes 13 tests, and usage documentation
passes 22 examples per safe/all mode. The explicit migration-completion gate
correctly reports 188 unfinished upstream source units; this is not full
upstream parity and the ignored campaigns were not rerun by this matrix.

Every pre-existing codec, test, script and dependency input still matches the
snapshot. Later additive benchmark and Module 15 oracle-preparation files are
not retroactively part of it. The nine added `reference_greedy_defaults` tests
have separate safe/chain/row/all checks; their complete frame and allocator
baselines also passed all sixteen clock-free benchmark configurations.

The [matrix evidence](evidence/2026-10-12-greedy-matrix.json) and
[archive](evidence/2026-10-12-greedy-matrix.tar.gz) preserve the runner, snapshot
identity/overlays, every command/log and independently recounted test totals.
The 99-member archive is 625,624 bytes and has been verified byte-for-byte.
