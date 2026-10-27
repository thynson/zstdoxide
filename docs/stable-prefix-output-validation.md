# Single-use prefixes in stable borrowed output qualification

The ownership/public, native, foreign-platform, complete corpus, full feature
matrix, measured performance and final source reconciliation gates are passed
and separately archived within this bounded prefix scope.

`Attachment<D>` is an ordinary owning value enum shared by all backends. Its
None, Persistent, PendingPrefix and ActivePrefix states move exactly one handle
or view without cloning it at frame start. No new pointer operation, interior
reference or allocation is introduced. Dictionary access and slice matches
retain their independent safe/unsafe implementations.

A successfully parsed standard header claims a pending prefix before ID and
resource checks. Standard completion releases it, including empty frames.
Skips and incomplete/rejected headers leave it pending. Failure retains an
active selection until reset/drop while denying output access. Reset retains
pending and persistent selections and releases active prefixes. Boundary-only
replacement preserves output, counters, scratch and limits; rejection changes
no session state and drops its input argument normally.

## Standalone ownership state

The standalone component has three ownership/lifetime tests under all 22
feature selections, plus no_std host object/IR and formatting checks. Separate
safe/all strict Clippy and warning-free object/IR checks are retained. This is
plain value-state verification, not complete decoder qualification. Nonclone
owners, exact drops, borrowed lifetimes, pending preservation and persistent
reuse are covered. An initial successful rustc invocation emitted a filename
warning, and a later logging collision is preserved separately from the final
successful commands.

## Private and public stages

The private integration campaign passes 18 commands and the public
constructor/replacement campaign passes 24, with separate source snapshots.
Each of five storage configurations passes 83 public/regression tests, eight
private session tests and 16 Rustdoc tests, plus strict Clippy. Three true no_std
configurations and formatting also pass. One explicitly ignored external test
is not counted as passed coverage. The original preparatory
assertion accidentally matched the unrelated dictionary sequence sink field and
prevented writing the main transformation; its failed compilation and exact
source are preserved. No incomplete run is counted as passed.

Private cases establish claim timing, skip preservation, empty-frame release,
raw/prepared output, prefix-once dependent concatenation and reset following
parsed versus unparsed failures. Public cases additionally exercise every split
and truncated prefix of an independent prepared frame, boundary replacement,
shared weak-owner release, partial parsing/body/checksum/skip phases, independent
nonclone dictionary and scratch allocators and each observed scratch failure.
Raw/RLE/empty/skip paths require no codec payload allocation. Rustdoc includes a
prefix view that cannot outlive its dictionary owner.

## Independent native prefix campaign

The native oracle calls `ZSTD_DCtx_refPrefix_advanced` with auto content detection
and enables stable output. It does not substitute persistent loaded/prepared
DDict selection for prefix use. The 125 descriptors at whole/one/seven-byte
input widths produce 750 native calls across normal and ASan/UBSan builds.
Native receipt `module27-native/native.json` reports all 750 passed.

Twelve repeated dictionary-dependent concatenation descriptors intentionally
reject after the first frame consumes the prefix. Six additional descriptors
prepend a skip or empty standard frame. Native streaming consumes its prefix
on a skip; Rust preserves an unclaimed prefix. Empty standard frames consume
it in both APIs. The established p>window native-success/Rust-CorruptData
policy difference remains explicit. Actual destination guards are checked;
successful bytes match independent originals. Malformed error precedence,
provisional bytes and opaque C reset/workspace ABI behavior are not claimed
equivalent. Downloaded reference source stays external.

The separate Rust replay has passed all five configurations: safe, isolated
slice, isolated dictionary, both and all. Each executes 750 calls using shared
and borrowed-view prefixes, for 3,750 Rust calls. Its source identities and
logs are bound to the independent native receipt.

## Foreign interpretation and concrete no-atomics code generation

`evidence/2026-10-26-stable-prefix-output-platform.json` records four strict-
provenance Miri runs on i686 and s390x, safe/all. Borrowed prefix content crosses
into current-frame history with actual guards, including prepared four-stream
literals, explicit boundary replacement, reset/reselection, short destinations
and expiry rejection. These are interpreter runs, not native foreign hardware.

Two thumbv6m core+alloc checks and two concrete release LLVM IR/ARM object builds
also pass. The retained probe actually constructs, processes and finishes a
borrowed prefix session. Objects are verified little-endian 32-bit ARM ELF;
this is code generation, not a linked/running bare-metal application.

## Complete dictionary corpus supplement

`evidence/2026-10-26-stable-prefix-output-corpus.json` records focused checks in
five storage configurations, strict Clippy/formatting, and complete safe/all
release replay of 12,063 files (353,411,091 bytes), with zero skips. The rolling
archive pin remains `4165df80490877b9b8f32dc0460d49896a362e192369cdc77ad1a0fd54e263e8`.
The original tail selectors and training sample choices are unchanged.

Each run includes 10,490 prepared and 440 loaded selections comparing direct,
stable and buffered output, plus 1,133 prefix selections comparing stable
prefix-once and buffered prefix output. Every stable path uses an explicitly
added seven-byte schedule and real fixed-destination guards. This is a streaming
supplement to a native one-shot target, not a newly introduced upstream selector.
A normal focused test additionally checks valid prepared prefix output, short
capacity, skip preservation and dependent-prefix reuse rejection.

Independent Rust training succeeds 11,768 times with 295 explicit empty fallbacks;
20 bounded decodes succeed and 12,043 reject malformed/limited inputs. Safe/all
per-input ledgers match each other and the prior direct/persistent campaigns.
Accepted fuzz errors are not valid-frame claims. Rust training is not native
fastCover, and the rolling corpus is not asserted membership in the v1.5.7 tag.
All member hashes, source, selectors and outcome ledgers are retained; corpus
payload remains external. C ABI/loading, original training and stateful context
contracts remain partial.

## Scope boundaries

The separately retained full regression and timing receipts below close this
bounded module. Complete upstream migration, overlapping in-place decoding and
unsupported C workspace contracts remain open.

## Claim, session-only reset and retry supplement

`evidence/2026-10-26-stable-prefix-output-lifecycle.json` separately records
48 native and 105 Rust lifecycle scenarios. Normal/sanitized native builds
exercise eight first-input kinds at whole/one/seven-byte widths; five Rust
storage selections exercise the seven cases with unchanged fixed capacity.
Native-only capacity replacement is retained as an observation, not emulated
through a Rust API that cannot resize its destination. Real guards and Rust
weak-owner checks remain active across initial processing, reset and retry.

Both retain a prefix after partial headers and parsing rejection, and consume
it after completed or empty standard frames. Native streaming consumes on a
skip, while Rust preserves the pending prefix. A valid header advertising more
output than the unchanged allowance is rejected before native prefix consumption,
but after Rust's documented claim point. Native can therefore retry with its
original prefix after session-only reset; Rust reset releases that claimed
handle and a later dependent frame requires an explicit new selection. These
are intentional lifecycle differences, with no C opaque-context equivalence claim.

## Complete regression, measurements and reconciliation

`evidence/2026-10-26-stable-prefix-output-feature-matrix.json`
records the complete suites: 1,628 passed and 61 explicitly ignored
tests across 91 result groups in each configuration. All twenty isolated
unsafe features and safe/all configurations have complete regression, true
no_std and strict Clippy qualification. Formatting and complete all-feature
release pass; safe release is also complete.
Ignored external or unsupported assertions are not counted as passed tests.

`stable-prefix-output-benchmarks.md` reports all 22 primary backends and independently rebuilt
safe/all repeats on identical deterministic workloads and allocation ledgers.
Source, fixtures, executables, CPU barriers and raw samples are retained. These
fixed-order host measurements do not establish a universal unsafe speedup.
`evidence/2026-10-26-stable-prefix-output-closure.json` rehashes this module's evidence and binds
the integrated production source to final matrix, native/platform, corpus where
applicable, and measured executable source identities. Previous milestones are
linked through their closure receipts; historical checks are not relabelled as
fresh executions. Complete upstream migration remains open.

The full matrix resumed on 2026-10-27 from the 55 completed commands of the
October 26 window. Fourteen pending commands were actually executed on the same
frozen source; prior logs and interruption receipts remain in the matrix archive.
Primary and independent repeat timings were recorded on October 27 after a fresh
CPU barrier. Evidence filenames retain the campaign origin date, October 26.
