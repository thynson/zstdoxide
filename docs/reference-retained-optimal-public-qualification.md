# Bounded retained Optimal public qualification

The Module 20 public extension admits resolved strategy 7 for known sources of
64..32768 bytes fitting the resolved window. Existing 64..16384-byte requests
retain their fresh dispatch and complete frame bytes. Larger admitted sources
use `ceil(n / 8192)` balanced blocks: the first `n % k` blocks contain
`floor(n / k) + 1` bytes, and the others contain `floor(n / k)` bytes.

Window logs remain 10..16, independent hash/chain logs 6..12, search logs 1..6,
minimum-match settings 3..7 and target lengths 0..4095. Zero request fields retain
automatic resolution semantics. Default levels 11/12 above 16 KiB may resolve to
BinaryTreeLazy2; the extension does not force them to Optimal. Search and price
state survive raw/RLE output. Only compressed publication commits candidate
repeat offsets and shared entropy state.

Retained Ultra/Ultra2, dictionary/prefix input, window expiry, numeric streaming,
long-distance matching and workers remain outside this extension. The complete
upstream migration remains open, including the existing 55 ignored obligations.

## Native and main public source

The immutable 2026-10-20 native campaign covers 1513 requests: 1433 predecessor
requests and 80 additions. All 2866 predecessor response byte strings remain
unchanged. The additions contain 61 retained Optimal successes, 8 previously
supported BinaryTreeLazy2 controls and 11 resource-limit rejections. The complete
campaign has 1361 successes and 152 resource-limit responses.

The new admitted requests produce 276 native decodes of Rust frames and 276 Rust
decodes of native frames, plus 156 malformed-request rejections. Normal and
sanitized native variants agree. The independent no-child offline auditor and
its controls are preserved with the full 3874-command campaign in
`evidence/2026-10-20-optimal-retained-public-native.{json,tar.gz}`.

The final main snapshot differs from that native candidate in two files only:
public Rustdoc in `reference_encode.rs` and one exact `dead_code` lint reason in
`reference_search.rs`. Removing only the documented Rustdoc lines, or substituting
that one diagnostic string, respectively, reproduces the final source exactly.
All other production source hashes agree. The new source-binding receipt retains
both preimages and exact final/native SHA-256 maps. No new native execution is
inferred from this reconciliation.

## Final public platform checkpoint (2026-10-21)

An additive probe calls the final public `compress_with_parameters_in` entry point
for a 16385-byte uniform source with W15/H6/C6/S1, minimum 3 and target 1. Safe/all
strict-provenance Miri runs pass on i686 (32-bit little-endian) and s390x (64-bit
big-endian). Each invocation checks compressed/RLE/RLE wire blocks, exact frame
consumption, complete source decoding, one surviving output allocation and full
release on drop. All 86 final production Rust source hashes are bound directly.

A separate additive concrete probe calls the public API with runtime source
input. Both safe/all thumbv6m no-atomics builds compile `core + alloc`, instantiate
the complete public frame path and retain LLVM IR plus ARM ELF objects. The six
commands all pass. These interpreted executions and code generation do not
constitute native foreign execution; the earlier private mixed raw/RLE/compressed
publication and reset qualification remains separate evidence.

Full snapshots, all logs, additive probes, concrete artifacts and native-to-final
source reconciliation are retained in
`evidence/2026-10-21-optimal-retained-public-platform.{json,tar.gz}`. Unchanged
inherited fixtures are omitted only after exact Git-blob comparison, with an
explicit revision/blob/size/SHA-256 reconstruction map.

## Final complete public regression matrix (2026-10-21)

All 22 full products pass with identical named outcomes: 1465 passed and 55
ignored per product across 69 named groups and 70 Cargo result sections.
The doctest group has separate ordinary and compile-fail result sections. The nineteen isolated unsafe-feature products and
release/all product are newly executed here. Main safe/all full regressions are
inherited only after exact source and log reconciliation. All 32230 passed
executions and 1210 ignored executions are repeated products of the same named
suite, not distinct test cases. The ignored upstream obligations remain open.

Nineteen isolated true no_std builds also pass again. Together with the exactly
bound main safe/all no_std builds, the reconciliation covers all 21 unique no_std
configurations. The 43 combined gates comprise 22 full products and 21 no_std
products. Every named test identity/outcome, all 86 production Rust files and the
complete shared snapshot inventory agree. The additive benchmark example has
separate formatting, Clippy and all-backend gates; it was not present in the
original full-suite snapshot.

The first reconciliation incorrectly expected 70 named groups and failed; its
source and traceback are retained unchanged. The additive successor verifies
69 named groups and 70 result sections in every product, without rerunning tests.

Original logs, source snapshots, exact named outcome reconciliation and executable
hash inventories are retained in
`evidence/2026-10-21-optimal-retained-public-matrix.{json,tar.gz}`. Test build caches
were released only after successful checks and inventory capture; their binaries
are not claimed as retained artifacts. Unchanged fixture bodies use explicit
Git-blob reconstruction records after byte-for-byte comparison.

The earlier formatting, strict Clippy, allocator, standalone usage and upstream
inventory checks remain source-bound in
`evidence/2026-10-20-optimal-retained-public-main.{json,tar.gz}`. No production
Rust changed during this session's final matrix and platform qualification.

## Remaining milestone gates

All-backend complete-frame timing, an independent safe/all repeat and the final
Module 20 closure receipt remain open. Clock-free qualification does not imply
performance measurements.
