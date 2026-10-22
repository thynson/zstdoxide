# Bounded retained Ultra public qualification

The Module 21 extension admits resolved strategy 8 for known sources of
64..32768 bytes fitting the resolved window. Requests of 64..16384 bytes retain
the existing fresh path. Larger sources use `ceil(n / 8192)` balanced blocks:
the first `n % k` blocks contain `floor(n / k) + 1` bytes and the rest contain
`floor(n / k)` bytes. This schedule is fixed before parser allocation.

Window logs remain 10..16, independent hash and chain logs 6..12, search logs
1..6, minimum-match settings 3..7, and target requests 0..4095. Zero request
fields retain automatic resolution semantics. Retained policy-2 search and
frequency state survive raw/RLE publication; only compressed publication
commits candidate repeat offsets and shared entropy state. Each source prefix
is exposed only when its block starts.

Retained Ultra2, dictionaries/prefixes, window expiry, numeric streaming,
long-distance matching and workers remain outside this extension. The 188
unfinished upstream migration units and 55 ignored test obligations remain open.

## Native interoperability and unchanged predecessor responses

The source-only manifest fixes 1593 requests: 1513 predecessor requests and 80
additions. It predicts 1423 successes and 170 resource-limit responses. Only
predecessor index 1511, `retained-resource-ultra`, changes from resource-limit to
success. Its two safe/all responses change as declared; the remaining 3024
predecessor response byte strings remain exactly identical.

All 62 newly admitted cases (61 additions plus that transition) have safe/all
Rust encoding, normal/sanitized native encoding, and complete decoding in both
directions. The campaign checks 248 native decodes of Rust frames and 248 Rust
decodes of native frames. Safe/all complete frames agree; normal/sanitized native
frames agree. No cross-implementation entropy-byte equality is required.

All 3964 capture commands pass, including 156 malformed-request calls with the
expected rejection exit status and no successful output. An independent offline
auditor reconstructs requests from the frozen source-only recipes and reconciles
every command, response, decoder output and predecessor identity. Ordinary and
optimized Python runs each reject 21 negative controls and accept two positive
controls, produce identical reports, and execute no children inside the auditor.

The capture reuses qualified pinned v1.5.7 native binaries only after checking
their exact identities and all used dependency identities before and after
execution. The source-binding receipt verifies identical inventories of 1968
files across focused public qualification, native capture, independent auditing
and final public platform qualification. No production-source transformation or
untested reconciliation is needed.

The complete source snapshots, logs, adapter packets, response files, native
identity records, controls and binding receipts are retained in
`evidence/2026-10-21-ultra-retained-public-native.{json,tar.gz}`. Unchanged fixture
bodies are deduplicated only after exact Git-blob comparison; reconstruction
records include revision, object identity, byte length and SHA-256.

## Focused main-source checks

Fourteen source-bound commands cover formatting, safe/all public boundary and
factory tests, strict Clippy, standalone usage, safe/all true no_std compilation,
the allocator support crate and the pinned upstream inventory. The inventory
still identifies 418 sources, 615 groups and 188 unfinished units; the expected
incomplete-migration check remains an explicit passing rejection control.

The first controller mistakenly required every allocator result group to contain
tests. Its 12 actual commands had succeeded, including 13 allocator unit tests
and a successful empty doctest group. The additive successor preserves that
failed controller, receipt and traceback, inherits the exact successful command
logs and source identities, then executes the two remaining inventory commands.
Inherited checks are not presented as reruns.

The focused checks above are separate from the completed full regression below.
Benchmark qualification is recorded in `reference-ultra-retained-frame-benchmark.md`.

## Final public foreign-platform checkpoint

The exact public source runs a complete 16385-byte uniform frame through
strict-provenance Miri on i686 (32-bit little-endian) and s390x (64-bit big-endian),
each with safe/all storage. All four executions check compressed/RLE/RLE block
history, exact frame consumption, complete decoding, one surviving output
allocation and full release on drop.

Safe/all thumbv6m checks compile the library against `core + alloc` without
atomics. Two additional concrete probes accept runtime source input and invoke
the final public compression entry point; their LLVM IR and ARM ELF objects are
retained. All eight commands pass. These are foreign interpreted executions and
concrete code generation, not native execution on those architectures.

All 86 production Rust files and the complete 1968-file public input inventory
match the native capture and its independent auditor exactly. The archive
`evidence/2026-10-21-ultra-retained-public-platform.{json,tar.gz}` retains the
source snapshots, effective environments, logs, probes, concrete artifacts and
source-binding receipt. The earlier private mixed-publication/reset platform
qualification remains separate complementary evidence.


## Complete public regression matrix

All 22 newly executed full products pass: safe, nineteen isolated unsafe
features, all features and release/all. Each product has 1505
passing tests and 55 ignored obligations, with identical
named outcomes and 72 Cargo result sections. This yields
33110 passed and 1210 ignored executions of the same suite,
not that many distinct tests. All 21 true no_std configurations also pass.

A separate pure reader reconciles all 1560 complete names across
71 groups, including five doctests and six should-panic names
omitted by the live controller's narrow token pattern. Ordinary/optimized reader
preflight each passes nine negative and two positive controls. Its first runner
had a syntax error before reading any test log; the original source and stderr
remain preserved alongside the corrected additive runner. No test was rerun or
changed for this reporting correction.

The 43 complete product gates retain exact command environments, logs, source
snapshots, outcome inventories and executable hashes. Ordinary Cargo caches
were removed only after successful checks and executable inventory capture;
those test binaries are not claimed as preserved measured artifacts. The
separate benchmark archives retain their actual campaign executables.

All 86 production Rust files, compiler/allocator/adapter inputs and every
predecessor input in the focused/native/platform snapshots match the final
matrix exactly. The matrix includes 1972 source inputs. The additive
benchmark example was absent from this snapshot and has explicit separate
integration and all-backend gates. Formatting, strict Clippy, usage, allocator
and upstream inventory checks remain bound to the exact same production source.

The archive is `evidence/2026-10-21-ultra-retained-public-matrix.{json,tar.gz}`.
Its SHA-256 is `50def8a8c49214dab24a247e8b5b043b7d938138fc940ea846ba94dd8406db61`. Independent archive reconstruction and a
second read of original files pass. The existing 55 ignored test obligations
and 188 unfinished upstream migration units remain open.
