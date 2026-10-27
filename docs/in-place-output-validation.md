# Bounded in-place decompression qualification

Module 28 remains in progress. Focused stages below are independently qualified;
complete feature/release regression, measurements and final reconciliation remain
separate gates. The implementation remains dictionary-free.

## Checked storage

The checked resumption constructor rejects reversed or out-of-range frame/output
coordinates before constructing history. It treats only the coordinator-supplied
written prefix as eligible history. Existing-frame match distances and window
limits remain enforced after reborrowing. No history clone is introduced.

`InPlaceBuffer` owns one exclusive slice and checked index coordinates. Raw blocks
permit actual overlap through safe `copy_within` or the independent checked
`unsafe-in-place-buffer` path. RLE protects the payload start. Compressed regions
use disjoint split borrows and checked endpoint publication. Rejected operations
preserve coordinates and perform no writes. Plain coordinates use one shared
representation; only raw movement selects the new unsafe feature.

The focused storage receipt records four safe/move/history/combined selections,
with nine history and six buffer tests in each, no_std, strict Clippy and format.
The private storage is initially test-gated until its coordinator is integrated.
Earlier standalone movement qualification is retained separately in the prior
session; final integrated platform/full-feature checks remain separate.

Evidence: `evidence/2026-10-27-in-place-storage.json`.

## Private frame coordinator

The frame loop copies metadata into values before writes. Compressed blocks
resume history in the mutable left split and retain their encoded payload in the
immutable right split, then publish the actual checked endpoint after borrows
end. Raw blocks permit overlap; all later encoded bytes remain protected. Frame
boundaries reset match eligibility, and frame-local output validates content size
and checksum. Scratch owners retain existing fallible allocation.

Six private-loop tests pass in safe/move/history/combined builds, including real
raw overlap, RLE/empty boundaries, native entropy fixtures at sufficient margin,
concatenation/skips, compressed blocks, malformed ranges and insufficient fences.
The coordinator remains test-gated pending the public API; complete regression
and independent native same-allocation qualification are separate.

Evidence: `evidence/2026-10-27-in-place-engine.json`.

## Public suffix-input API

Four public entrypoints expose default/options and Global/caller-selected scratch
allocation. Seven public contract tests, six loop tests and the Rustdoc example
pass in safe/move/history/combined builds with no_std and strict Clippy.
Exact margin, buffer identity/capacity, all unused bytes, invalid starting indices,
window and output limits, malformed mutations/truncations, frame-local history
and every exercised scratch-allocation failure ordinal are checked. Borrowed
non-Clone allocator ownership is exercised separately. No complete input/output
copy is used by the decoder; copies in test setup restore original fixtures.

Evidence: `evidence/2026-10-27-in-place-public-api.json`.
Usage and limitations: `in-place-output-usage.md`.

## Independent pinned native overlap comparison

The external helper uses facebook/zstd v1.5.7 revision
`f8745da6ff1ad1e7bab384bd1f9d742439278e99` as a test oracle only. It executes
the original `tests/fuzzer.c:1344-1397` single/two-frame input construction with
5 MiB RDG input, seed 0/compressibility 0.5, and negative maximum-block-size level.
Expanded input, the documented sufficient-margin bound, original output capacity
(including the two-frame case), exact decoded size and full original bytes are
asserted. Both source and destination point inside one real guarded allocation.
Source attribution is retained under the repository `LICENSE.zstd`.

The ordinary and AddressSanitizer/UndefinedBehaviorSanitizer helpers agree on
1,300 layouts covering valid/malformed raw, RLE, compressed, concatenated and
skippable frames, truncations, mutations, tight gaps and output allowances.
All outer guards and configured output allowances are checked. LeakSanitizer is
unsupported on this macOS host: its startup rejection is preserved, and the
actual sanitized run disables only that unsupported facility.

Five Rust capture configurations (safe, move, history, combined and all) agree
with each other. All 1,291 nonempty native verdicts agree, and every common success
output matches byte for byte. Nine zero-input layouts intentionally preserve the
existing Rust `UnexpectedEof` versus native empty-concatenation success difference.
Native modifies some unused successful destination suffixes (21 observed layouts);
Rust independently verifies its stronger unchanged-unused-suffix contract.

Both final ignored adapter tests are explicitly executed in all five configurations,
including the original full-size assertions. An ordinary full suite still reports
these external adapters as ignored; it does not silently count them as passed.
Evidence: `evidence/2026-10-27-in-place-native.json`.

A separately labelled supplement also replays every existing official decodecorpus
frame at exact sufficient margin, verifies full output and unused tails, and checks
one-byte-short output allowances across five backends. It retains the original
generator fixtures and metadata unchanged. Evidence: `evidence/2026-10-27-in-place-corpus.json`.

## Foreign interpretation and concrete code generation

The final API executes actual raw overlap, compressed split-borrow history,
concatenated frame reset, insufficient bounds and both guards under strict-
provenance Miri on i686 and big-endian s390x in safe/all builds. All four executions
pass. Safe/all thumbv6m core+alloc builds and instantiated LLVM IR/ARM objects
also pass; the retained IR contains the actual memmove operation. These eight
commands prove interpretation/code generation, not native foreign hardware
execution. Evidence: `evidence/2026-10-27-in-place-platform.json`.
