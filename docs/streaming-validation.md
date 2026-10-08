# Module 6 streaming validation

Reference: facebook/zstd v1.5.7 at
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`. Measurements and local checks were run
on 2026-10-08, with Rust 1.89.0 on `aarch64-apple-darwin` unless stated otherwise.
This report distinguishes implemented ordinary streaming contracts from later
dictionary, parameter, stable-buffer, allocator, worker, CLI and legacy work.

## Implementation checks

Safe ring history and its independent `unsafe-ring-history` backend share ten
unit tests covering wraparound, overlap, zero space, invalid distances, retained
capacity, allocation failure, counters beyond 32 bits, and overflow rejection.
The unsafe ring tests also passed Miri on the installed nightly (10 tests,
64.59 seconds). Every unchecked ring access has a checked caller and a local
proof; the store helper documents its full safety preconditions.

Incremental XXH64 matches independent C-generated vectors at all 131,841 split
points over 513 prefixes, plus irregular updates of a 1,000,003-byte input.
One-shot and incremental state share scalar round functions while retaining
independent input chunk handling. The state allocates no heap storage.

The block-encoder refactor preserved all 48 recorded complete pre-refactor
compressed outputs byte-for-byte. Reusable literal/sequence/bitstream workspaces
have a capacity-stability test. Eight focused encoder state tests cover header
widths, output drainage, pledges, omitted content size, reset, invalid options,
latched source suffixes, and checksum verification. The four decoder phase tests
passed default testing and all-feature Miri, including the independently found
and fixed post-EOF misuse/poisoning inconsistency.

Both exact compressed-block regressions also passed all-feature Miri through
the public streaming API: history overwrite (17.53 seconds) and small sequence
sections (11.06 seconds), including their fragmentation schedules.

A separate review ran 9,216 fragmented mutation/truncation/output-budget cases
against one-shot decoding: success/failure agreed and successful bytes matched.
This bounded review campaign is additional evidence, not a maintained corpus or
an exhaustive safety proof. A separate official-CLI smoke campaign decoded 192
streamed Rust frames to exact source bytes across data/size/window/flag settings.
The committed oracle target provides the maintained repeatable compatibility test.

## Committed conformance tests

`tests/upstream_streaming.rs` contains 39 normal tests, including five shared
producer checks, plus an explicitly selected official-CLI oracle test. Tests
exercise real incremental APIs; they do not implement chunking around a
whole-frame decoder. Exact source mapping and remaining assertions are in
[upstream-streaming-plan.md](upstream-streaming-plan.md).

Coverage includes the exact 47-byte history-overwrite and 51-byte small-sequence
regressions, early impossible magic rejection, raw output before block completion,
zero-capacity calls, RLE drainage, precise concatenation boundaries, checksum
latency, EOF at every proper prefix, poisoned-state recovery, and limit reset.
All 96 preserved complete decodecorpus frames run under two independently
fragmented schedules, while all 16 header-only assets fail EOF validation.

Encoding checks exercise live-decoder Flush visibility, resubmitted Finish
suffixes, pledge zero versus unknown size, both wrong-pledge directions with
content-size emission on and off, and repeated deterministic context reuse.
The 128-seed directive campaign executes each schedule twice across five data
profiles. The malformed streaming target exercises 4,096 arbitrary inputs and
3,200 valid/truncated/mutated schedules using the upstream tail-driven producer.
Unsupported random parameters/stable-buffer modes retain partial source status.

`tests/upstream_window.rs` replays 20 independently generated near-window frames,
with 10,000 source-derived sequences each. The original normal window sweep
through log 26 and large-test sweep through log 29 both passed. Logs 10–20 run
normally; the explicit large-window test covers 21–29 and passed release builds
with both safe and all unsafe backends. The largest case retains a 512 MiB
window while caller output remains 64 KiB. Generation reproduces byte-for-byte,
and C streaming decoding verifies exact all-zero output before fixture import.
The offline integrity audit now covers 263 files in six collections.

## Complete local matrix

`sh scripts/check.sh` passed 207 tests with 11 deliberate ignores in each of ten
runs: safe defaults, each of seven unsafe features, all features in debug, and
all features in release. This is 2,070 passing executions of 207 tests, not
2,070 independent contracts. Safe, individual, and combined no_std compilation
passed in all nine configurations. Formatting, both all-target Clippy runs with
warnings denied, source/case inventory and fixture-integrity checks passed.

The matrix log SHA-256 is
`cdbe64ff3be41d6a0efeae85fca9bc86fa094e2f08a5acb1afc448db7c8fa892`.
The frozen streaming test source hash during that run was
`46c9fc9b20b9ccca2588620e0a27740f96ffca93cc351aa4e8155cec51b26708`.
Both examples in [streaming-usage.md](streaming-usage.md) also passed a separate
`rustdoc --test --edition=2024` run against the built safe library.

CI now includes the seventh backend, ring/decoder Miri, near-window fixture
regeneration and extended replay, and the official streaming interoperability
test. Local passing runs do not imply a hosted CI execution has occurred.

## Memory and throughput

The nine-configuration, eleven-workload measurement matched every output and
capacity fingerprint. Encoder retained storage plateaued at 1,352,289 bytes over
four versus 64 blocks. Decoder retained storage plateaued at 264,107 bytes while
one frame grew from 512 KiB to 8 MiB; another trial retained 509,902 bytes over
four versus 64 concatenated frames. These are context capacities observed after
calls, excluding temporary scratch, inline state, caller buffers and allocator
overhead. They are not peak process memory or allocation-call measurements.

The independent unsafe ring feature was 0.945x the safe throughput in this run;
all-feature streaming decode/encode were 1.116x/1.075x. The recorded source hash,
sample ranges, workload sizes and limitations are in
[benchmarks.md](benchmarks.md) and the linked raw data. Unsafe remains opt-in.

## Independent oracle orchestration

The updated `scripts/check_upstream_oracles.sh` completed successfully against
the pinned external source/build. Every fixture collection reproduced, including
all 20 large-window sequence frames. Seven explicitly selected test functions
passed: two cross-codec interoperability tests, golden encoder compatibility,
4,096 newly generated decodecorpus frames, 30,000 one-shot fuzz cycles, streamed
cross-codec compatibility, and the complete large-window decoder sweep. This
also revalidates the one-shot encoder workspace refactor under the long campaign.
The known failing legacy success assertion is deliberately not selected.

## External streaming corpus replay

The selectable ignored `external_official_streaming_corpus` adapter replays the
pinned rolling `stream_decompress` and `stream_round_trip` release assets. The
source and parameters are consumed by the source-derived tail producer and
streaming driver. Unsupported random compression settings, stable output and
in-place behavior remain explicit partial-port qualifications.

| Corpus | Files | Source bytes | Safe release | All-feature release |
| --- | ---: | ---: | --- | --- |
| stream_decompress | 12,462 | 80,339,212 | Passed | Passed |
| stream_round_trip | 24,223 | 469,442,481 | Passed | Passed |

Every file ran with no skips. External input is capped at 2 MiB because the
largest round-trip input is 1,048,688 bytes. The output/window limits are
10 MiB/128 MiB, and file count is bounded at 65,536. Archives and extracted data
remain outside the repository. All ten pinned external archives also passed
offline re-extraction and inventory comparison: 53,081 files, 811,198,036
unpacked bytes, and 157,546,348 compressed bytes. These snapshot identities are
independent of the tagged source revision.

The external runner was added after the frozen matrix. Its shared helper
parameterization passed both safe and all-feature normal streaming suites
(39 tests), strict Clippy and audits, then both complete corpus replays in each
backend configuration. The ordinary suite remains 207 passing tests; the new
opt-in runner raises deliberate ignored entry points from 11 to 12. No
production codec changed after the full matrix.
