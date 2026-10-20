# Bounded Ultra2 public qualification

The main checkout now implements strategy 9 within the independently qualified
fresh no-dictionary envelope. Private feature/platform gates and focused main
admission checks are complete. Full public regression and benchmark campaigns
are separately tracked; Module 19 is not yet complete and this report does not
claim their unfinished outcomes.

The resolved bounds are source length 64..16384, contained in the selected
window, W10..16, H/C6..12, S1..6, minimum-match 3..7 and target 0..4095. Default
levels 16/17 fit 64..2048-byte sources. The public plan always enables the initial
discarded pass and automatic literal prices. It rejects invalid resource bounds
before allocator cloning. Tiny private eligibility controls do not enlarge the
public envelope. Retained blocks, dictionaries, prefixes, window expiry, numeric
streaming, long-distance matching and workers remain outside this milestone.

## Candidate frame and ownership checks

A copied `Ultra2FramePlan` contains checked geometry, frame settings and the raw
frame bound. Static dispatch constructs the nine allocator owners in the same
order as the private coordinator. The initial pass preserves frequency state,
invalidates history through checked coordinate rebasing, clears token lengths
and restores original repeats. Only the completed second pass enters the shared
entropy encoder; the first pass publishes no frame or entropy state.

The isolated candidate passes focused public bounds, targets, three-byte-key,
complete frame, failure, borrowed nonclone-owner and preallocation rejection
checks in safe and all-feature builds. Prior strategy integration tests, dispatch
and frame-plan checks, formatting, strict all-target Clippy and true no_std pass
in eleven commands, with 600 unchanged compiled input identities. The frame
archive is `evidence/2026-10-20-ultra2-public-frame.tar.gz`, SHA-256
`f32bfebef765e860407e7bed4e0dc9d687fbe5bae3408dffbebd47a708a78105`.

A later targeted full-regression check exposed one obsolete legacy expectation:
level 16 with an empty input now resolves to implemented Ultra2 and returns
`ResourceLimit`, rather than `UnsupportedFeature`. Its request and source are
unchanged. The isolated regression successor changes only this test expectation;
all four affected integration tests pass. The original failure and three
cancelled incomplete full-suite attempts are preserved. They are not counted as
completed full regression products. Production source bytes remain identical.

## Foreign-target qualification

Two public tests pass under strict-provenance Miri for each safe/all build on
i686 little-endian 32-bit and s390x big-endian 64-bit targets: eight interpreted
test executions. They exercise complete frames, borrowed nonclone allocators,
source/output lifetimes, decoding and rejection before allocation. These are
foreign-target interpreter results, not native foreign executions.

The thumbv6m target also passes core-plus-alloc checks and concrete public
compression/shared-entropy/result-drop release code generation for safe/all
builds. LLVM IR and ARM ELF objects are retained. All eight platform commands
pass on the same 600 candidate inputs. The archive is
`evidence/2026-10-20-ultra2-public-platform.tar.gz`, SHA-256
`5a32e1202ca8018b04bc0be82aecdd18184c4ca2a96d75d4f225264bf258cc35`.

## Usage and migration checks

All 26 standalone documentation checks pass in safe and all-feature builds,
including new default and explicit Ultra2 examples and existing compile-fail
ownership contracts. The usage candidate binds every production Rust source and
Cargo input. The initial package omitted declared bench files and failed before
compilation; its exact logs and inputs remain archived alongside the successful
successor. No implementation change was needed. Archive:
`evidence/2026-10-20-ultra2-public-usage.tar.gz`, SHA-256
`d880b872bfc40d4f25e42a2c2de001195d0cef18dc1c3adeecc5e4888782a4c9`.

Allocator tests, strict Clippy, formatting and the pinned upstream inventory pass.
The distinct `--require-complete` migration gate remains nonzero, as expected for
unfinished migration. An initial isolated support package lacked manifest target
files; the successful successor checks the actual repository. Both attempts are
retained. The post-check target inventory is explicitly supplemental, not an
invented pre-check snapshot. Archive:
`evidence/2026-10-20-ultra2-public-support.tar.gz`, SHA-256
`ac9aa15b76f9f7fccce1e2e2885b145b8e153a153119823778f8e9de78346b88`.

## Completed native public interoperability

The immutable isolated candidate completes 16,008 commands for 1,433 recipes:
1,292 admitted and 141 resource-limited, with no unsupported result. Normal and
fully ASan/UBSan-instrumented C builds each compile all 26 relevant codec units.
Both C builds decode safe/all Rust frames, and both Rust builds decode normal/
sanitized C frames: 5,168 frames in each direction and 802,569,512 decoded bytes.
The 156 malformed-adapter exits are expected exit 2. Rust safe/all outputs and
normal/sanitized C outputs agree within their respective implementation pairs.
All 4,888 predecessor successful responses retain their exact bytes; C and Rust
are not required to produce identical compressed representations.

Independent positive audits and all 59 adversarial controls pass in normal and
optimized Python without child execution. The lossless archive retains 48,594
regular files and 2,587,742,073 logical bytes; complete reconstruction and a
second unchanged-original verification pass. See
[public interoperability](reference-ultra2-public-interop.md) for its source
closure and archive identity. The separately frozen 26-case native frame baseline
is in `tests/fixtures/reference-ultra2-frame-baseline`; owner surveys and timing
remain pending.

## Full regression in progress

The first nine complete products (safe, unsafe-byte-cursor, unsafe-bitstream,
unsafe-history, unsafe-ring-history, unsafe-fse, unsafe-fse-encoder, unsafe-huffman
and unsafe-huffman-codebook) pass 1,435 tests each, with
zero failures and 55 explicit ignores. Independent
parsing of the first three raw Cargo logs finds 67 groups and identical individual test
names/outcomes, including the separate merged and compile-fail doctest groups.
Each completed product also passes true no_std. Remaining isolated features,
all-features and release/all remain running or pending; these nine products do
not constitute the final full matrix. Original cancelled attempts stay retained.

## Main-checkout admission

After the complete private backend/platform gate, main received the exact 90
production identities qualified by native interoperability and public foreign
target checks. All original main hashes were verified before copying, and
previous versions were retained. The independently corrected legacy assertion
and already verified usage documentation accompany the same production bytes.

All eleven focused main commands pass: formatting; safe/all public, frame-plan
and dispatch checks (28 passing test executions); strict all-target Clippy; and
true no_std for safe/all. All 1373 recorded source/input identities, including
every fixture format and the new benchmark example, remain unchanged throughout
this qualification. The new example had already passed an isolated safe/all
26-case native-frame/owner survey without clocks. The complete 21-backend survey
is a separate gate. No full-regression or timing completion is inferred here.

Evidence: `evidence/2026-10-20-ultra2-public-admission.tar.gz` and its adjacent
archive report (SHA-256
`16cac9de147567c4e1ea703154a0a7b88d4d38c03318e615f6b90770487dd3a9`).
The independently reconstructed archive preserves the complete
qualification and exact Git-blob references for inherited fixture bytes.
