# Caller-owned compression output validation

Module 33 is in progress. Bounded output and internal one-block frame emission
are integrated. Public APIs, native/corpus/platform checks, full-codec regression
and whole compression measurements remain open.

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

## One-block frame emission

The internal ordinary encoder shares a fully initialized 17-byte maximum header
record with the existing allocating path. It stages one encoded block, appends
that block through FrameSink, clears/reuses staging, and writes the checksum.
It owns no full-frame output vector. Existing BlockEncoder history, parser and
entropy operations remain unchanged. The plain header record needs no unsafe
representation. Allocation failure releases every scratch owner; a later failed
append can leave the previously completed header/blocks visible.

Thirteen focused safe/sink/all 22 commands pass. Six new units cover independent
wire vectors, exact existing-compressor output at content-size/block boundaries,
raw/RLE/compressed patterns, round trips, output guards/tails, every short small
destination, below-bound exact fits, every observed scratch failure and borrowed
non-Clone allocator owners. One inherited all-ID/all-size header/allocator unit
also runs in each configuration, with true no_std, strict Clippy and formatting.
The initial test used an options value with the older scalar-limit convenience
API; that compile rejection is retained and not credited as a pass.
Evidence: `evidence/2026-10-27-slice-encode-engine.json`.
