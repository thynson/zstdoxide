# Persistent dictionaries in stable borrowed output qualification

`SliceDecoder` accepts a persistent shared dictionary or borrowed view while
holding one fixed exclusive output borrow. Dictionary preparation and scratch
allocation owners are independent. A temporary sequence sink borrows dictionary
metadata and current-frame output without creating interior references or
cloning the selected handle. Prepared Huffman/FSE fallback and initial repeat
offsets are restored at each standard header; skippable frames preserve the
selection. ID checks precede window/content limits. Source fragments are never
retained.

Reset retains the dictionary, output borrow, limits and encoded-input capacity,
while releasing frame-local Huffman/FSE overrides. `retained_capacity()` remains
scratch-only; `dictionary_retained_capacity()` separately reports reachable
payload rather than unique ownership or shared-control allocation. Prefixes and
attachment replacement are outside this bounded module. Errors poison the
session and deny output access until reset; bytes outside its allowance remain
unchanged. The existing dictionary expiry rule and independent unsafe dictionary
and slice-history backends remain in force.

## Focused lifecycle and public qualification

The sink and private integration snapshots each pass 17 focused commands;
public admission passes 24. Safe, isolated slice history, isolated dictionary,
their combination and all features each run focused integration/unit tests and
strict Clippy. Public qualification also runs Rustdoc, true no_std and formatting.
The evidence binds exact source identities and preserves each distinct stage.
Six public tests cover every split of
a prepared HTTP frame, raw/prepared fixtures, empty calls, exact/short limits,
window limits, persistent concatenation with skips, checksum/truncation/mutation
failures and successful reset recovery. Weak-owner tests verify the consumed
shared handle keeps its dictionary alive until the session drops. Borrowed
nonclone dictionary and scratch allocators remain independent; every observed
scratch failure ordinal is replayed, and raw/RLE/empty/skip sessions require no
codec payload allocation. Rustdoc includes a dictionary-view lifetime failure.

## Independent native stable-output campaign

`evidence/2026-10-26-stable-dictionary-output-native.json` records 119 descriptors
at whole, one-byte and seven-byte input widths. Native loaded and prepared DDict
sessions, both normal and ASan/UBSan, produce 1,428 calls with actual output
guards. Rust shared and borrowed-view sessions run the same 357 cases in five
storage configurations, totaling 3,570 calls. Exact successful bytes match
independent originals, and unused Rust suffixes remain unchanged. The native
helper enables ZSTD_d_stableOutBuffer and uses the pinned v1.5.7 decoder.

Cases include raw/prepared content, omitted/mismatched IDs, treeless literals,
repeated entropy and duplicate offsets, dictionary-to-frame crossing, persistent
concatenation, zero/exact/short/excess destinations, truncation and checksums.
The p>window fixture explicitly expects native success and Rust CorruptData;
this established policy difference remains visible. Malformed-input error order,
provisional bytes and C context workspace layout are not asserted equivalent.
Downloaded upstream source stays external; hashes, helper source, executables,
inputs, results and Rust replay logs are retained.

## Foreign execution and concrete code generation

`evidence/2026-10-26-stable-dictionary-output-platform.json` records four strict-
provenance Miri runs: safe/all on i686 and s390x. Real dictionary-origin matches,
forward crossing, prepared four-stream literals, repeated reset/decode,
concatenation, short-output rejection and expiry exercise guarded destinations.
These are interpreter executions, not native foreign hardware measurements.

Two thumbv6m core+alloc checks and two concrete release code-generation builds
pass. LLVM IR contains `stable_dictionary_output_codegen_probe`, which creates
a dictionary view, constructs a public session, processes input and finishes.
Retained objects are verified little-endian 32-bit ARM ELF. Production source
hashes match the candidate; the probe is separately recorded.

## Explicit fixed-fragment corpus supplement

The dictionary_decompress adapter retains every original tail selector and its
independent Rust training path. Persistent prepared/loaded branches additionally
run seven-byte stable sessions with actual destination guards and poisoning
checks, comparing success/bytes against direct and buffered output. This fixed
input schedule is an added supplement to a one-shot native target; it is not an
original native streaming selector. Prefix branches remain buffered. Five
storage configurations, strict Clippy and formatting pass focused checks.

The complete 12,063-file pinned corpus (353,411,091 bytes) now passes safe/all
release with zero skips, as recorded by
`evidence/2026-10-26-stable-dictionary-output-corpus.json`. Its archive SHA-256 is
`4165df80490877b9b8f32dc0460d49896a362e192369cdc77ad1a0fd54e263e8`.
Per run, 10,490 prepared and 440 loaded selections exercise stable/direct/buffered
comparison; 1,133 prefix selections retain the buffered path. Independent Rust
training succeeds 11,768 times and uses 295 explicit empty fallbacks. Twenty
bounded decodes succeed and 12,043 reject malformed/limited data. Safe/all
per-input ledgers match each other and the separate Module25 direct campaign.
Every corpus member hash, branch, source size, capacity, training and outcome is
retained. The corpus is a separately pinned rolling snapshot, not asserted
membership in the v1.5.7 source tag.

The final feature matrix, performance comparison and closure are separately
recorded below.
Opaque C context initialization/reset, arbitrary attachment replacement, prefix
use, allocator/workspace ABI behavior and complete upstream migration remain
outside this bounded milestone.

## Empty-block boundary supplement

`evidence/2026-10-26-stable-dictionary-output-empty-block-boundaries.json`
records five hand-encoded boundaries independently from the 1,428-call native
campaign above. Empty raw/RLE blocks and a complete empty compressed block with
unknown content size succeed. A compressed body larger than a known zero window
and a compressed body missing its mandatory literals/sequence sections fail.
Pinned native streaming skips a zero-byte compressed body; native one-shot and
Rust reject it. This permissive native shortcut is an explicit interoperability
difference, not a claim of malformed-input parity.

The regression passes all 22 retained backend libraries, with absent/persistent
empty dictionaries, actual guards and direct/whole/one/seven-byte stable paths.
Safe/all strict Clippy and formatting also pass. It is added before the complete
feature matrix; production source is unchanged. The initial standalone compiler
selection failure and corrected invocation are both preserved.

## Complete regression, measurements and reconciliation

`evidence/2026-10-26-stable-dictionary-output-feature-matrix.json`
records the complete suites: 1,612 passed and 60 explicitly ignored
tests across 88 result groups in each configuration. All twenty isolated
unsafe features and safe/all configurations have complete regression, true
no_std and strict Clippy qualification. Formatting and complete all-feature
release pass; safe release is also complete.
Ignored external or unsupported assertions are not counted as passed tests.

`stable-dictionary-output-benchmarks.md` reports all 22 primary backends and independently rebuilt
safe/all repeats on identical deterministic workloads and allocation ledgers.
Source, fixtures, executables, CPU barriers and raw samples are retained. These
fixed-order host measurements do not establish a universal unsafe speedup.
`evidence/2026-10-26-stable-dictionary-output-closure.json` rehashes this module's evidence and binds
the integrated production source to final matrix, native/platform, corpus where
applicable, and measured executable source identities. Previous milestones are
linked through their closure receipts; historical checks are not relabelled as
fresh executions. Complete upstream migration remains open.
