# Reference-stream parameter lifecycle validation

Module 41 remains in progress. Its implementation and individual qualification
items are WIP checkpoints until all module gates pass. Module 40's receipts
qualify its earlier source, not the subsequent API additions.

## Focused public behavior

The reference streaming suite now has 17 passing tests. Six additions cover
transactional request/options replacement, rejected profile/pledge admission,
all configuration stage gates, family changes with fresh-versus-reused byte
equality, parameter resets, reset-all, and failure recovery. Raw and formatted
prefixes are exercised across clean, active, finishing, finished and failed
states. Allocator snapshots prove setters/resets do not allocate or alter
ownership; completed-frame parameter reset preserves counters and the applied
profile. The first allocation after default reset is rejected and recovered;
the existing suite also exhausts each fresh-session allocation ordinal.

Safe/all focused tests, strict Clippy and no_std checks pass as recorded in
the setters/reset focused receipts. The fixed-profile compiled suites also
pass (7 frame tests and 8 selection tests). Miri is skipped under the standing
safe-only-change directive; no unsafe operation was added or modified.

## Guarded native schedules

The new ignored adapter is run explicitly for 72 frames: six admitted families,
six source/pledge/prefix cases, and two sessions. Operations include clean
parameter/all resets, intermediate request replacement, active bulk-request and
parameter-reset rejection, reset-all recovery, pending-prefix preservation and
completed-frame parameter/session reset. Each resulting frame cross-decodes.
Normal and ASan/UBSan pinned v1.5.7 oracles produce identical frames and rows;
Rust safe/all exporters produce byte-identical frames and schedules, and both
Rust backends decode native output. Caller output guards and dictionary-free
native header windows are checked. Sanitizers cover C only and leak detection
is disabled. No native byte equality or formatted-prefix profile identity is
claimed. The helper is an external oracle, never a codec dependency.

Native completion returns to init automatically; Rust requires explicit
session reset. Parameter-only native reset can retain a fresh pledge, whereas
Rust restores default options and clears it. The analysis probe records that
difference explicitly; the frame adapter reapplies the intended pledge after
reset. Native individual mid-frame setters/worker updates are outside scope.

See `evidence/2026-11-05-reference-streaming-parameters-native.json` and its
archive for source hashes, commands, guards, frame artifacts and raw logs.
This campaign does not complete the repository's upstream test migration.

## Intermediate feature and platform regression

All 81 planned intermediate commands pass on the frozen final source.
Safe/all and all 22 isolated features run the 32 focused streaming/selection
public tests, true host no_std, and strict library/all-test Clippy. Safe/all
release configurations run those same 32 targeted tests. Safe/all atomic-free
ARM checks and concrete release LLVM IR/object emission pass. Formatting and
the complete safe/all suites also pass: each full suite reports 130 result
groups, 1,884 passed, 82 ignored and zero failures. The new native ignored
adapter is qualified separately, not credited from its skipped ordinary run.

All-feature no_std explicitly lists the 22 unsafe flags with defaults disabled;
`--all-features` would otherwise also enable std. The intermediate receipt and
archive record command lists, source hashes, output-object hashes and logs.
These checks do not replace full isolated-feature suites or full release
suites. No Miri ran because only safe code changed.

## Lifecycle timing

The separate comparison records 15,360 samples across four lifecycle modes,
alternating admitted requests, two text sizes and safe/all independent repeats.
All outputs and ledgers match the fresh warmed reference exactly. See
`reference-streaming-parameters-benchmarks.md` for the complete-frame timing
protocol, measured ratios and limits; no universal unsafe speedup is inferred.

## Additional full suites

Completed full release suites: `safe-full-release`, `all-full-release`.
Completed full isolated-feature suites: none.
Every credited suite reports 130 groups, 1,884 passed, 82 ignored and zero
failures. The additional-suites receipt records any incomplete attempt and
the remaining feature list. Time-bound termination is not counted as a pass.

## Remaining module gates

22 full isolated-feature suites remain.
The module remains open until every required suite has passed on the same
source and the receipts are reconciled. Completed focused/platform/native/
timing checks do not substitute for an unrun full suite. No implementation
milestone or full upstream replacement is claimed.
