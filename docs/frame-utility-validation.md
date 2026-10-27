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
