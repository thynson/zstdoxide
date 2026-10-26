# Caller-owned one-shot output qualification

The four `decompress_into*` APIs write directly through checked `SliceHistory`.
They share frame parsing with the existing owned decoder, use no output vector
or history copy, and keep compressed entropy/literal scratch fallible. The
contract and exclusions are in `caller-output-design.md` and public Rustdoc.
Module 23 remains in progress: complete feature/platform regression and measured
safe/unsafe comparisons are separate gates.

## Focused API and ownership checks

`evidence/2026-10-23-caller-output-frame-loop.json` binds the shared-loop refactor
to safe/all old decoder, malformed-input, upstream fuzzer and decodecorpus
regressions, true no_std, strict Clippy and formatting.
`evidence/2026-10-23-caller-output-public-api.json` records nineteen commands
covering safe, isolated `unsafe-slice-history`, and all-feature builds. Each runs
seven destination/allocator tests, old regressions, documentation examples and
an input/output-alias compile-fail example, no_std and strict Clippy.

The destination tests exercise exact/short/zero capacity, untouched success
suffixes, known and unknown content size, raw/RLE/compressed blocks, combined
options limits, independent advertised-window limits, concatenation/skips,
truncation, dictionary rejection and corrupted checksums. Owned tracking and
borrowed nonclone allocators prove scratch release; each observed compressed
scratch allocation ordinal is failed separately. Raw/RLE paths issue no codec
payload allocation requests. Allocator cloning keeps the allocator's own
semantics and is not claimed allocation-free.

## Pinned native destination and external corpus checks

`evidence/2026-10-23-caller-output-native.json` and its hash-bound archive retain
helper source, native compile commands and executables, the pinned upstream
source manifest, generated cases, outputs, guard results, Rust logs, source
snapshots and per-file external corpus hashes. The 90 pinned upstream library
source/header/assembly hashes were checked against the actual reference checkout.
Native helpers remain external test oracles and are not linked into the codec.

- Normal and AddressSanitizer/UndefinedBehaviorSanitizer native builds each ran
  401 common destination cases plus two null/zero-output cases: 806 executions.
- Rust safe/all each ran the same 401 common cases: 802 executions. Successful
  bytes match independently generated originals; prefix/suffix guards survive.
  Insufficient native destinations report `dstSize_tooSmall`, and corresponding
  Rust calls report `ResourceLimit`. Error-time provisional output is not compared.
- The common cases cover 96 official decodecorpus frames, two golden fixtures,
  empty and skippable frames, a concatenation, corrupt checksum and truncation.
  Exact, excess, zero and one-byte-short capacities are selected without duplicates.
- A normal committed test now checks all 96 generated frames through direct
  output with exact/excess/short destinations, including cross-block history.
- Safe and all-feature release builds each replayed every one of the 1,173
  hash-pinned destination-too-small corpus files, with no skips. The adapter
  retains exact tail-selected source/capacity semantics, checks a real mutable
  destination and guard bytes, and also retains the earlier allocating-limit
  assertion. The archive SHA-256 is
  `2ad245fd1f679882ceac61745c1c05cb33515c747f807bca60559ca8e1fd6b9d`.

The corpus is a separately pinned rolling release snapshot, not proven part of
v1.5.7. Its inputs remain outside the repository. Original level/context and
sequence-producer branches remain partial; complete corpus execution does not
make the whole upstream target port complete. The inventory audit against the
pinned checkout passes, without `--require-complete` or a parity claim.

## Remaining gates

Isolated-feature whole-suite regression, all-feature full release and
deterministic allocation/performance comparisons remain open until their own
evidence is recorded. Stable incremental destination
contracts, overlapping in-place decoding and caller-owned dictionary output
are outside this bounded module.

## Foreign interpretation and no-atomics code generation

`evidence/2026-10-23-caller-output-platform.json` records four strict-provenance
Miri executions: safe/all on i686 and s390x. Independently generated compressed
frames exercise exact destination output, bounded rejection, sentinel protection
and concatenation with new frame-local history. These are 32-bit and big-endian
interpreter checks, not native foreign hardware runs.

Two thumbv6m no_std core+alloc checks passed, and two release code-generation
builds retain LLVM IR with a concrete `caller_output_codegen_probe` definition
and verified 32-bit little-endian ARM ELF objects. The probe calls the actual
public options-based direct-output API. The source snapshot changes only the
probe addition; production source identities remain bound to the working tree.

## Resumed feature and complete safe/all regression

`evidence/2026-10-26-caller-output-feature-matrix.json` binds 71 successful
commands to the source/test identities and archived logs. The archive contains
code and text metadata; binary fixture bytes remain at the recorded Git commit,
with every file hash retained in `complete_source`. Twenty isolated
unsafe features plus safe/all each passed the three destination integration
suites, true no_std compilation and strict Clippy: the original 66-command
prefix. The five additive commands complete safe/all full regression, safe/all
release destination tests and formatting. Both full suites report 1,563 passed
and 56 explicitly ignored tests across 77 result groups, including Rustdoc.
Ignored external/unsupported assertions are not counted as passed coverage.

The original campaign was paused at the user's request during safe full-suite
compilation on October 23. Its running record, pause receipt and incomplete log
are preserved unchanged in the archive. New October 26 logs prove completion;
the interrupted command is neither a failure nor a passing execution. Source
hashes were checked before and after the additive campaign. Whole suites for
every isolated feature and the full all-feature release suite remain a separate
qualification item.
