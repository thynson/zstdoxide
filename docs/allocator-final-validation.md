# Final allocator codec matrix

The integrated Module 9 codec was validated in an isolated archive of
`8703ff4d74f9a87b40ab63a491408848de198a0c`. All 631 archived files retained their
original hashes after every command; the helper build generated only its own
Cargo.lock. The coordinator verified that committed `src`, `tests`, and
`test-support/allocator` files still match that revision. The later allocator
benchmark entry and harness have separate [validation](allocator-benchmarks.md).

The compiler was `nightly-2026-10-08`, reporting Rust 1.101.0-nightly, commit
`1d81eb4ad9cd207e3e638bd32b17ec4fce8412a6`, on aarch64-apple-darwin. The independent
oracle reported Zstandard CLI v1.5.7 from the pinned external source checkout.

## Executed checks

All 58 recorded validation commands passed.

| Check | Observed result |
| --- | --- |
| Complete default codec suite | 699 passed, 50 ignored |
| Each of 12 individual unsafe-feature suites | 699 passed, 50 ignored per configuration |
| Complete all-feature debug suite | 699 passed, 50 ignored |
| Complete all-feature release suite | 699 passed, 50 ignored |
| Aggregate of the 15 complete suites | 10,485 passed executions; 750 ignored executions |
| Separate allocator helper | 13 tests, formatting and strict Clippy passed |
| Standalone usage examples | Ten selected examples passed in each of safe and all-feature configurations, including expected lifetime compilation failures |
| no_std compilation | Safe, every individual unsafe feature, and all 12 together passed |
| Main crate formatting and Clippy | Formatting and strict safe/all-feature all-target Clippy passed on the archived codec |
| Upstream inventory integrity | 418 test-related sources, 221 explicit exclusions and all 615 named anchors verified against the pinned source |
| Imported fixture integrity | 312 files across eight collections verified |

Counts are test executions, not distinct upstream behavioral assertions. Ignored
cases are not passing cases. The explicitly selected oracle functions below run
separately from the ordinary suites.

## Independent official-codec comparisons

Safe and all-feature runs each selected ten public C-oracle test functions.
Each configuration made the following complete decoded-byte comparisons:

| Group | Rust frames decoded by C | C-to-Rust output comparisons |
| --- | ---: | ---: |
| Whole-buffer interoperability | 31 | 56 |
| Streaming interoperability | 20 | 10 |
| Dictionary encoding | 116 | 0 |
| Raw dictionary training | 16 | 16 |
| Formatted dictionary training | 36 | 36 |
| Custom-allocator one-shot encoding | 36 | 0 |
| Custom-allocator stream encoding | 18 | 0 |
| Custom-allocator stream decoding | 0 | 16 |
| Per configuration | 273 | 134 |

The custom decoder row produces eight official frames and decodes each through
borrowed and shared allocator contexts. Thus there are 126 generated C-frame
cases and 134 Rust output comparisons per configuration. Across safe and all,
20 test-function executions perform **814 exact byte comparisons**. The evidence
records the case-count formulas rather than equating function counts with frame
coverage. External C is only a test oracle, not the codec implementation.

## Separate evidence and remaining migration

This matrix does not repeat the large 89,456,640-byte source campaign, long
Miri ownership sweeps, or platform follow-ups. Their original records remain
linked from [allocator validation](allocator-validation.md),
[failure coverage](allocator-failure-validation.md), and
[platform validation](allocator-platform-validation.md). These matrix runs are
correctness checks, not codec timing measurements.

The separate `audit_upstream_tests.py --require-complete` command still exits 1:
188 source units remain unfinished, and external corpus target coverage remains
partial or blocked. Source dispositions are 14 ported, 34 partial, 149 blocked,
five planned and 216 non-applicable. Named-case dispositions are 47 ported,
106 partial, 447 blocked and 15 non-applicable. Passing inventory integrity
validates the mapping; it does not make the complete-migration gate pass.

The [consolidated record](evidence/2026-10-09-allocator-final-matrix.json) contains
all commands, counts, log hashes, compiler/oracle identity and the separately
classified incomplete-migration result. The
[complete evidence archive](evidence/2026-10-09-allocator-final-matrix.tar.gz)
contains full logs, manifests, runner and records. No test result or benchmark
has been inferred from an unexecuted selector or from source inspection alone.
