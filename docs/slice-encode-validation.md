# Caller-owned compression output validation

Module 33 is in progress. The bounded output leaf is integrated; frame emission,
public APIs, native/corpus checks, full-codec regression and measurements remain
open. No caller-owned compression API is claimed yet.

## Checked frame sink

FrameSink owns an exclusive initialized output slice and a checked position.
Every append validates addition/capacity before mutation, preserving the complete
failed append and current position. Earlier writes remain visible. Default safe
copies and opt-in unsafe-frame-sink copies have identical contracts; unchecked
copy_nonoverlapping runs only after runtime extent checks, with complete nearby
validity/alignment/initialization/alias/lifetime proof. No allocation is owned.

Exact leaf/error source passed 15 isolated component commands: safe/unsafe host
four-test suites, core-only compilation and strict Clippy; four strict-provenance
i686/s390x Miri executions of the same four tests; safe/unsafe core-only thumb
checks and concrete ARM copy objects; formatting. Tests exhaust small append
pairs with guards, recover after short destinations, cover empty slices and check
numeric overflow without manufactured huge slices. ARM IR instantiates memcpy
without a Rust allocator entrypoint. Foreign hardware execution is not claimed.

Seven additional full-codec compile/lint/format commands pass in safe, isolated
sink and all 22-unsafe-feature selections. Clippy does not count as test execution.
The private declaration temporarily allows dead code until frame emission uses
it. Complete whole-codec tests, release, every feature and measurements are later
milestone gates. Separate isolated sink measurements compare safe/unsafe primary
and independently rebuilt repeats after actual CPU barriers; see
`frame-sink-benchmarks.md`. These are not full compression measurements. Evidence: `evidence/2026-10-27-frame-sink-component.json` and
`evidence/2026-10-27-frame-sink-integration.json`.
