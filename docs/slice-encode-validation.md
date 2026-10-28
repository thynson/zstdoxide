# Caller-owned compression output validation

Module 33 is in progress. Caller-owned ordinary compression APIs and bounded
one-block frame emission are integrated. Native/corpus/platform, full-codec
regression and whole compression measurements remain separate gates.

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

## Caller-owned public APIs

compress_into and compress_into_in emit exactly the ordinary dictionary-free
Greedy frame produced by compress. A successful result is the exact written
length; its unused output suffix remains unchanged. Capacity may be smaller than
compression_bound if the actual frame fits. ResourceLimit can leave provisional
header/blocks; no whole-call atomicity is promised. Independent borrows prohibit
in-place compression. Generic scratch ownership supports borrowed non-Clone
allocators and releases all owned allocations before return, including failures.
Dictionary, reference-level and streaming variants remain separate.

Twenty-one focused commands execute four public contracts, six engine units and
two documentation examples (one compile-fail alias check) under safe, isolated
sink, isolated match-table and all 22 features, plus true no_std, strict Clippy and
formatting. Tests check complete single-frame metadata/output, short-capacity
guards and immutable input, borrowed allocator failure/recovery, and header
rejection before any scratch allocation.
Evidence: `evidence/2026-10-27-slice-encode-public-api.json`.

## Native interoperability and deterministic corpus

Thirty-six fixed inputs span empty/short sizes, content-size field thresholds,
block boundaries and three-block payloads using RLE, repeated and deterministic
noise patterns. Safe/all exports preserve identical inputs and exact Rust frame
bytes. Independent normal and ASan+UBSan v1.5.7 helpers decode all Rust frames and
produce their own frames. All 324 native bounded-capacity attempts check real
outer guards, immutable input and complete successful decompression. Each native
capacity sweep is relative to its own encoded size; native byte/capacity/error
identity with Rust is not assumed. Native successful rows: 59;
rows modifying an unused success suffix: 32.
Rust separately enforces its unchanged success suffix. macOS leak detection is
unsupported and disabled; no leak-sanitizer claim is made.

Four safe/sink/match/all Rust replays decode all 36 native frames, reproduce every
Rust frame and check 324 per-backend capacity/result/guard observations.
Two strict native-adapter Clippy checks pass. All 347 retained deterministic fuzz
inputs preserve their original tail-selected source and allocating-output/decode
assertions, adding bounded success/short-output comparisons in safe/sink/all.
This supplement does not port the native random-capacity selector, unsupported
advanced parameters or persistent-context branch. No complete external rolling
corpus or original-group promotion is claimed.
Evidence: `evidence/2026-10-27-slice-encode-native.json` and
`evidence/2026-10-27-slice-encode-corpus.json`.

## Foreign interpretation and concrete emission

Four strict-provenance Miri runs on i686/s390x in safe/all configurations execute
actual borrowed compression, exact allocating-output parity, metadata and decode
for empty/raw/RLE/compressed input, including short-destination guards. Four
thumbv6m checks/object builds use core plus alloc and instantiate actual
compression/output copying in retained LLVM IR and ARM objects. These are
interpreter/code-generation checks, not execution on foreign hardware.
Evidence: `evidence/2026-10-27-slice-encode-platform.json`.

## Complete feature and release qualification

All 75 commands pass on the final frozen candidate: safe/all and every 22 isolated
unsafe feature each execute complete tests, true no_std compilation and strict
Clippy; safe/all release and formatting also pass. Each complete/release suite
reports 1740 passed and 73 ignored across 111 result groups. Native adapters are
explicitly executed in their separate focused capture/replay receipts.

The final candidate deliberately combines qualified public/native/platform/corpus
files and the independently audited post-32 inventory reconciliation. Earlier
focused snapshots retain their original metadata; none was silently rewritten.
Integrated production and all matrix source identities are rechecked before this
item is committed. Whole compression measurements and closure remain open.
Evidence: `evidence/2026-10-27-slice-encode-feature-matrix.json`.
