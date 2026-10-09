# Configurable named match-search validation

Module 11 is in progress. The [design](match-search-design.md) defines the
operational controls and resource boundaries. These checkpoints do not add
numeric reference strategies or complete the upstream test migration.

## Independent default compatibility inputs

The pre-change codec snapshot `ba4529c` produced 76 frames under each of the
safe and all-feature backends. Twelve deterministic workloads cover all three
named policies, fresh and reset contexts, strategy cycling, retained-history
boundaries, dictionary seed thresholds and short Flush crossings. All 152
frames were independently decoded by pinned C with complete byte comparisons.
The original recipes, records and reproduction sources are retained in the
[baseline archive](evidence/2026-10-09-match-search-baseline.json).

The saved output lengths and FNV fingerprints are compatibility regression
inputs. They are distinct from the original complete C byte comparisons.
Historical capacities describe the capture host and include all codec storage
plus the selected dictionary; no search-only capacity is inferred from them.

## Frozen hash primitive observations

The [match-search collection](../tests/upstream/match-search/README.md) calls the
actual pinned C pointer wrappers for key widths four through eight and hash
logs ten through eighteen. It records 2,880 primary observations, 2,304
unused-byte invariance pairs with width-eight sensitivity controls, and 64
historical four-byte arithmetic guards. Every C input has eight initialized
readable bytes. Future Rust extraction must read only the selected key width.

Normal regeneration, two explicit reproduction configurations and ASan/UBSan
reproduction agree byte-for-byte. The importer verifies all 20 included pinned
headers and separates stable provenance from capture compiler/archive metadata.
It rejects an attempted report overwrite and a corrupted isolated fixture even
with Python assertions disabled. Independent root checks confirm that the
generic offline audit rejects fixture mutation, importer mutation and a missing
required collection. The current audit passes 324 files in ten collections;
the oracle runner's shell syntax also passes.

[Oracle evidence](evidence/2026-10-09-match-search-oracle.json) preserves all
sixteen checkpoint/review commands, full logs and reproduction reports. The
recorded C platform is little-endian arm64 macOS with 64-bit `size_t` and
32-bit `int`. This checkpoint runs no Rust hash implementation and promotes no
whole upstream test case. C headers and the linked oracle library remain
outside the repository and are not production dependencies.

## Validated options and storage shape

`MatchSearchOptions` now validates each concrete scalar setting without
allocation; unsuccessful setters preserve the complete record. The private
`SearchKeyLayout` validates shifts and key dimensions, while `SearchShape`
computes effective history, policy effort and all cached requested bytes using
the actual target layouts. The aggregate admission ceiling is 32 MiB of requested
payload. It is not a peak-memory, retained-capacity or allocator-overhead limit.
An empty one-shot frame resolves to zero history. No live setting is exported
or applied at this foundation checkpoint.

Seven focused tests pass in safe, all-feature, no_std safe and no_std with all
twelve unsafe features. Both no_std library checks, strict library/all-target
Clippy with all features, and owned-file formatting pass. All seven tests also
pass under strict-provenance Miri on i686 and big-endian s390x using cached
sysroots. These fourteen interpreted executions validate shared safe scalar
arithmetic and target layouts, not native foreign-platform or unsafe-backend
execution. The checkpoint adds no unsafe operations or allocations.

[Record evidence](evidence/2026-10-09-match-search-records.json) embeds sixteen
command logs and the frozen source hashes. It retains the initial all-target
Clippy failure caused by concurrent baseline-test compilation, and the final
passing run after that test owner repaired the errors. Search owner resizing,
hash primitives and actual parser/context consumers remain later items.

## Offline default-frame replay and archive audit

Five regression/schema tests and the two existing RDG helper tests now replay
every captured default frame. Each of safe, all-feature, no_std safe and no_std
with all twelve unsafe features passes all seven tests: 304 complete current
frames and 608 comparisons with the unchanged original backend records. Inputs
and dictionaries are verified before encoder calls. Output fingerprints, frame
and block metadata, process-call counts, bounded progress and output canaries
match; every current frame decodes completely in Rust. Fresh/reset frames and
the first/final Greedy strategy-cycle frames also match byte for byte. Portable
capacity assertions check reset preservation and reuse of both cache families.
Focused strict Clippy, no_std library compilation and owned formatting pass.

`scripts/check_match_search_baseline.py`, registered in both check runners,
verifies the original archive and every member, reproduction-source attribution,
exact TSV bytes and current RDG/dictionary inputs. It neither extracts the
archive nor regenerates codec expectations. Normal Python and `python3 -O` each
pass one positive isolated audit and twenty expected negative rejections,
including altered TSVs with locally updated hashes and invalid archive members.
The script uses explicit errors, so optimization cannot disable its checks.

[Replay evidence](evidence/2026-10-09-match-search-defaults.json) links all
commands, logs and the isolated mutation reproducer. This checkpoint does not
rerun C: the original archive retains the 152 independent complete C decodes.
Future implementation changes must continue satisfying these saved inputs.

## Exact-width copied keys and hashes

`MatchKey` carries an initialized little-endian word and explicit width, with
no source reference or allocation. Extraction copies only four through eight
selected bytes and returns `None` for a shorter slice. Hashing rejects a width
mismatch before applying the pinned wrapping arithmetic. Four-byte array
conversion preserves existing private callers; keys may be reused at another
hash log only when their width remains compatible. This implementation is
entirely safe and shared by all access backends.

Six tests pass in each of four safe/feature/no_std configurations. Each replays
all 2,880 C vectors, 2,304 unused-byte pairs with full-width controls and 64
historical compatibility guards, with complete tuple/count checks. Strict
library Clippy, no_std compilation and owned formatting pass. Three focused
tests pass under strict-provenance Miri on the host, i686 and s390x: nine
interpreted executions covering exact five-/six-/seven-byte allocations,
unaligned sources, short tails, width identity and frozen endian/wrapping
witnesses. No physical eight-byte source read is required for shorter keys.

[Key evidence](evidence/2026-10-09-match-search-keys.json) embeds fourteen command
logs. The initial i686 automatic-sysroot cache permission failure ran no tests;
the recorded successful foreign-target runs use existing sysroots and an
isolated `f08fceb` foundation with the identical key source. No target installation
or source change was required. These tests establish primitive correctness;
dynamic leaf storage and real policy consumers remain separate checkpoints.

## Dynamic Greedy candidate table

`MatchTable` now owns a checked active layout alongside its positions. Default
constructors retain the old dimensions. Reconfiguration validates and reserves
through the exact owner before clearing entries or publishing metadata;
in-capacity shrink/regrow makes no allocation request. Seed copying checks both
layouts and initialized lengths before mutation. Every safe or unsafe replacement
checks key width and its computed index against live length, including malformed
shortened internal test states with retained spare capacity. Existing block
callers now propagate these checked errors; benchmark inputs remain unchanged.

All 22 recorded commands pass: fourteen leaf tests in six configurations,
three no_std checks, strict all-target Clippy safe/all, two exact release bounds
checks, safe/all default-frame replay and six exact strict-provenance Miri
checks. This totals 106 passing test executions. The new cases cover all 45
hash-log/key-width combinations, seed/key mismatch, grow failures, shrink/regrow,
borrowed non-Clone owners, independent allocation domains and every request in
the selected constructor/two-growth campaign. Default replay still verifies all
76 logical frames per backend against the original records.

[Table evidence](evidence/2026-10-09-match-table-layouts.json) embeds command logs,
the frozen owned source and the integration source hashes shared with the
candidate-chain validation. Concurrent root commits are recorded accurately;
this focused checkpoint does not claim an immutable whole-repository matrix.

## Dynamic bounded candidate chain

`MatchChain` now uses the checked layout for its heads and admits up to 524,288
cyclic nodes. Default constructors preserve the existing key/hash dimensions.
Traversal accepts up to 64 examined links, including future and stale links;
zero remains an internal empty traversal. Tagged insertion order and rejection
of stale or non-descending links are unchanged. Seed transfer validates layout
and live head lengths, then preserves the exact slot/tag ordering and the
existing equal-size/larger-destination replacement rules.

Reconfiguration validates both allocation layouts and reserves both retained
owners before changing logical state. A later reservation failure may leave
earlier capacity growth, but every initialized record, length, tag and active
layout remains valid. Success explicitly initializes the new prefixes and starts
empty traversal state. All unsafe head/node accesses retain runtime bounds
checks against initialized length and updated alias/lifetime proofs.

Twenty-three tests pass in seven configurations: safe, individual unsafe-chain,
all features, no_std safe/all and release safe/all. Both no_std library checks,
strict all-target/all-feature Clippy and owned formatting pass. Five exact
all-feature strict-provenance Miri checks exercise live bounds, both reservation
failure points, borrowed owner reuse, incompatible seeds and tiny wrapped rings.
Safe/all default-frame replays pass all seven tests and all captured frames.

[Chain evidence](evidence/2026-10-09-match-chain-layouts.json) preserves all logs
and six unchanged integration source hashes. It also retains transient test
compilation during the neighboring table migration and a runner correction:
the successful default suite has seven tests, including two shared helper tests,
which the first driver had incorrectly expected to be five. No passing codec
run is represented as failed codec behavior or silently rerun for that count.

## Width-aware parser probes and explicit effort

Current-position, Lazy lookahead and consumed-start probes now extract typed
keys at the table's immutable selected width. Incomplete tails remain literals.
The verified emitted-match minimum stays four, demonstrated by an eight-byte
hash collision whose sources agree for only four bytes. The explicit Lazy entry
point and Chain accept budgets zero through 64; the default Lazy wrapper keeps
sixteen. Both Lazy probes receive the selected budget. Lookup/insertion order,
distance-one opportunities, current-match ties, full-tail behavior and once-only
consumed-start insertion retain their existing contracts.

Thirty-two parser tests pass in five feature/std configurations. Ten public
strategy regressions pass across safe/all, with one existing C oracle explicitly
ignored in each. Thirty-six one-shot and 108 streaming default frame fingerprints
per build remain unchanged. Strict library/test Clippy, no_std compilation and
formatting pass, as do three exact all-feature strict-provenance Miri tests.
The checkpoint totals 177 passing executions and two deliberately ignored oracle
executions across twenty successful commands.

[Parser evidence](evidence/2026-10-09-match-search-parser.json) preserves source
and dependency hashes and all logs. It explicitly corrects an evidence-summary
wording error: leaf and parser checkpoints replayed saved default frames in
Rust; the original independent C capture belongs to `ba4529c`, not a new leaf
C run. No current configurable public encoder or C strategy equivalence is
claimed before the remaining context consumers are integrated.

## Borrowed dictionary, history and input keys

`MatchSource` now extracts every selected width across its borrowed dictionary,
wrapped retained history and current input. Both region endpoints are valid
incomplete tails; out-of-region positions fail. Indexed extents use checked
`usize` arithmetic while absolute frame positions remain checked `u64` values.
Dictionary-to-frame crossings require retained frame zero, independently from
ordinary dictionary-only match eligibility. Extraction copies only initialized
selected bytes into stack scratch and never allocates or changes an owner.

Seven new tests pass in six feature/std configurations, with three existing
boundary regressions passing in safe/all builds. Both no_std library checks,
strict all-target/all-feature Clippy and formatting pass. All seven helper tests
also pass under all-feature strict-provenance Miri; one exact coordinate test
passes under i686 Miri with frame positions beyond 32 bits. This totals 56 passing
test executions across 19 commands. Miri execution is interpreted, not native
foreign-target execution.

[Source-key evidence](evidence/2026-10-09-match-source-keys.json) embeds all command
logs and verifies the unchanged source hash. Existing four-byte helpers and
consumers remain intact at this checkpoint. No new C oracle or complete archived
default replay was run for this addition; storage and live-control integration
remain subsequent work items.

## Frame-local search storage and dictionary seeds

Block preparation now resolves the selected shape before mutation or allocator
cloning. Active Greedy/Chain owners reshape through their exact retained
allocators, while dictionary seeds use the selected hash layout and key width.
Chain seeds retain exactly the newest `min(S, D-W+1)` complete starts when `D>=W`.
Short or missing dictionaries clear both seed families, and stale inactive
layouts are excluded from regular seed copying. Complete boundary keys remain
eligible. Parser probes receive selected widths and concrete Chain/Lazy budgets.

Default insertion order and entropy publication remain unchanged. A separate
search-capacity measurement sums exactly the seven potential search vectors,
including inactive caches; total capacity reuses that sum with other workspaces.
Reset preserves the plain search record. Controls remain private until the next
history checkpoint consumes the selected horizon and the public boundary is
implemented. This checkpoint intentionally retains 128 KiB history.

Eight new storage tests pass in seven native feature/std/release configurations.
They cover maximum dimensions, all width-dependent seed thresholds and tiny
private ranges, short-dictionary replacement, actual effort-dependent selection,
every observed warmed-growth allocation failure, balanced exact owners, empty
frame preparation and entropy commit/discard. Safe/all replay all 76 archived
default frames, and eight existing ownership/entropy regression executions pass.
Both no_std checks, strict all-target/all-feature Clippy and formatting pass.
The checkpoint totals 78 passing executions across 22 successful commands.

[Storage evidence](evidence/2026-10-09-match-search-storage.json) preserves logs,
eight source hashes and two independent production reviews. The initial Lazy
test recipe accidentally shared the next key too, correctly preventing delay;
only that recipe was repaired. The reviewed production bytes stayed identical.
No new C process or maximum-size Miri campaign ran at this integration step;
leaf Miri and original default-frame C evidence retain their separate scopes.

## Selected output-history horizon

The block encoder now reserves `min(history_bytes, frame_window)` on the first
nonempty block, using the ring's exact retained allocator. The shared one-MiB
maximum is constructor metadata only. An empty frame does not construct a ring,
and defaults keep their 128-KiB bound and existing allocation behavior. Smaller
subsequent selections reuse earlier high-water capacity. Dictionary eligibility
still follows the frame window independently from retained output.

Seven new tests pass in eight feature/std/release configurations. Tiny one-,
two-, seven- and eight-byte horizons wrap across width-eight keys and short
blocks; every retained suffix is compared byte for byte, including reset reuse.
All three policies find a distance-196,608 source only when both selected history
and the wire window permit it. Regular dictionary matches remain available
after frame-zero eviction, then stop after their independent window boundary.
Six increasing ring reservations each fail and recover with the same exact
owner. Borrowed-owner shrink/regrow makes no unnecessary request, and output
failure leaves reserved history uncommitted.

Safe/all archived defaults still pass all 76 frame/capacity fingerprint cases;
storage and existing history/failure regressions also pass. Both no_std checks,
strict all-target/all-feature Clippy and formatting pass. The final 23 commands
total 92 passing test executions. [History evidence](evidence/2026-10-09-match-search-history.json)
embeds source hashes, logs and reproduction scripts, retaining initial test-only
API and owner-count corrections. The production delta never changed after its
publication. No additional C, Miri or timing execution is claimed here. All
private search consumers are now operational; public boundary wiring follows.
