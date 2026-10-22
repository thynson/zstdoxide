# Bounded retained Ultra2 implementation qualification

## Dedicated constructor capability

The dedicated `Scope::Ultra2Retained` constructor consumes the existing three
parser owners for policy 2 and physical source lengths 14..32768. It resets them
without changing allocator identity. The configuration accessor rejects used,
poisoned or initialized-frequency state. Every previous parser/provider entry
rejects this scope before observation or search mutation. It does not yet admit
retained Ultra2 execution or change public strategy-9 responses.

Eight focused tests (four existing retained Ultra and four new retained Ultra2)
pass in all 22 products: safe, nineteen isolated unsafe features, all features
and release/all features. The same nine public predecessor integration tests
pass separately in safe and all-feature builds. All 21 true no_std products,
formatting and strict all-target/all-feature Clippy pass. The evidence includes
changed Rust sources and complete command output. Unchanged predecessor fixture
payloads are pinned to exact Git blobs and SHA-256 digests, with a checked restore
script in the archive; full snapshots also remain in the external stage. Rust test executables are
identified by hashes and remain in the recorded target directory, with their
contents explicitly excluded from this archive. This is a focused capability
qualification, not a replacement for the final complete regression matrix.

Evidence: `docs/evidence/2026-10-22-ultra2-retained-constructors-compact.{json,tar.gz}`.
Archive SHA-256: `a0467d552fc894d6b63842cf5a54ab6214f4872e2824b106b6a220d272f8af9e`.

## Checked schedule and first-block coordinates

`RetainedUltra2Plan` copies two-to-four block lengths, validates the whole
physical source/window and all nine existing owner byte bounds, and reserves
complete frame output before owner allocation. The first block determines the
single logical rebase; a skipped first-block bootstrap keeps origin zero.
Checked visible-prefix coordinates accept only exact schedule boundaries and
reject future-source and stale-index access. The old fresh and retained layout
validators keep their original bounds. A prospective public frame plan balances
16385..32768 bytes into three/four blocks, without changing public admission.

All five focused plan tests pass in 22 products, including every proposed public
length, all 32 native schedules, first-block thresholds and non-power-of-two
biases, exact owner/output bounds and invalid geometry. All 21 true no_std
products, formatting and all-target strict Clippy pass. Safe and all-feature
predecessor integration runs each pass the same nine tests. This remains a
focused qualification, with changed Rust sources, complete logs and executable
hashes retained. Unchanged predecessor fixture payloads are recoverable through
the archived exact-commit Git blob and SHA-256 manifest/restore script; complete
source snapshots also remain in the external stage.

Evidence: `docs/evidence/2026-10-22-ultra2-retained-plan.{json,tar.gz}`.
Archive SHA-256: `fb3a6795d0d6babfb1b2935f9e811d4e54c1fac65b782057498f8463bc80a2c5`.

## Executable storage and build-resource qualification

Constructor executables have been moved from generated target locations into
separate lossless gzip files after independent streaming reconstruction of every
size and SHA-256. Their exact paths, hashes and modes are recorded in
`docs/evidence/2026-10-22-ultra2-retained-constructors-executable-storage.json`.
This receipt supersedes the original target-location statements; source snapshots
and command logs are unchanged. An earlier memory-heavy chunk archive was
interrupted before removing any executable; its helper, log and partial output
remain external history. Regenerable incremental/object intermediates were also
removed, with executable identities verified before relocation.

The first plan attempt stopped during compilation because the disk filled; no
plan test ran in that attempt. Its source/logs are retained. The next focused
attempt passed all five tests, and the final complete matrix passed with explicit
`CARGO_INCREMENTAL=0`, `CARGO_PROFILE_DEV_DEBUG=0` and
`CARGO_PROFILE_TEST_DEBUG=0`. These settings affect build artifacts and symbols,
while preserving the tested feature combinations and debug assertions.
