# Checked frame utility qualification

Module 31 is in progress. Focused gates below are complete; full feature/release
regression, measurements and final reconciliation remain separate gates.

## Scalar bounds and checked serialization

The pinned one-pass bound formula uses a width-independent repeating-0xff00 input
limit and checked arithmetic. It returns typed values/errors instead of native
zero/error sentinels. Skippable serialization checks variant, u32 payload length,
header addition and actual capacity before writing a fixed little-endian header
and copying payload once. Rejection preserves all destination bytes; successful
writes preserve unused suffixes. Plain scalars and transient slice copies add no
independent raw-memory data structure. The module forbids unsafe code.

Six unit tests run in safe/cursor/all configurations with no_std, strict Clippy and
formatting (10 commands). They preserve original bound powers/oversized assertions,
check independent wide raw-framing inequalities over every small input, and cover
pointer-width numeric limits without fabricating huge slices. Initial snapshot
copy/registration failures remain retained. The corrected driver verifies exact
test counts. Its final format-label parser initially raised after all ten commands
passed; source/log identities and exact counts were independently revalidated and
the controller corrected. No rejected or zero-test attempt receives pass credit.
Evidence: `evidence/2026-10-27-frame-utility-core.json`.

## Public contracts and encoder scope

Two public functions expose typed bounds and caller-owned writing without an
allocator parameter. Six unit, five public and three documentation tests pass
in safe/cursor/all configurations alongside no_std, strict Clippy and formatting
(16 commands). The alias compile-fail example proves overlapping immutable payload
and mutable destination borrows are rejected. Actual ordinary/raw/prepared
one-shot outputs, admitted reference profiles and small-window raw fallback fit
the bound and decode exactly. All variants, empty/large payloads, exact/short/extra
capacity, real guards and standard/skippable concatenations are exercised.

The bound excludes arbitrarily flushed streaming output and multiple independent
frames. It is neither workspace size nor an allocation-success guarantee. Initial
missing-registration compilation and the same final label-parser bookkeeping
rejection remain retained; completed final command/source identities and exact
test counts were independently verified before recording success.
Evidence: `evidence/2026-10-27-frame-utility-public-api.json`.

## Native boundaries and exact original writer history

Normal and ASan/UBSan v1.5.7 helpers execute original test_compressBound scalar
assertions plus 280 supplemental numeric cases and 343 real guarded writer layouts.
All normal/sanitized rows and destination bytes agree. Three Rust backends replay
every scalar result and every full destination/guard allocation byte.

One maximal-admitted scalar input yields a raw native formula/function result of
usize::MAX-2. The C encoded-error convention classifies it as an error; Rust retains
the typed numeric bound. The initial adapter's assumption that every native error
had a zero formula failed and is archived. Revised captures retain raw function,
macro and classified results separately. Production source did not change.
In 58 layouts both variant and capacity are invalid: native reports capacity first,
while Rust documents InvalidParameter first. Both reject before mutation. Neither
difference is credited as identical native error semantics.

The original fuzzer runs unchanged through both 129 KiB and empty skippable
write/recognize/read assertions, with read-only capture of actual preceding seed,
payload and output. Normal/sanitized captures agree. Three Rust backends reproduce
every encoded byte and original read-length/variant/payload assertion. Additional
guard, metadata and decompression checks are explicitly supplemental. All 176
external lib/programs/tests C/header/assembly files match the pinned archive.

This macOS ASan runtime rejects leak detection. That rejected launch is preserved;
all layout operations were rerun with leak detection disabled and bounds/undefined
behavior checks enabled. No dynamic leak-detection pass is claimed. Original-history
capture exits after the asserted region and leaves process setup owners to exit.
Evidence: `evidence/2026-10-27-frame-utility-native.json` and
`evidence/2026-10-27-frame-utility-history.json`.

## Width, endianness and concrete objects

Eight actual strict-provenance Miri runs cover i686/s390x safe/all: each runs two
numeric-limit unit tests and two real borrowed-write/guard tests. This executes
the 32-bit header-addition overflow branch without creating invalid giant slices.
A host core-only preflight, safe/all thumbv6m core-only checks and actual LLVM
IR/ARM objects also pass, for 13 commands. Instantiated bound/write probe IR has
no Rust allocator entrypoint. Exact production utility/scan/cursor/frame/error
modules are copied into the component. Full-codec support remains no_std plus
alloc; these are interpreter/code-generation results, not foreign hardware runs.

The first component template omitted utility registration and failed before
Miri test execution. Corrected registration and added host preflight preceded
all final successful commands; original source/logs remain archived.
Evidence: `evidence/2026-10-27-frame-utility-platform.json`.
