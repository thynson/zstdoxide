# Official decodecorpus replay

## Design and ownership

The source is facebook/zstd v1.5.7, commit
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`, `tests/decodecorpus.c`.
The upstream executable is a fixture generator and independent test oracle,
never a codec fallback. Fixtures retain the generator's exact encoded bytes
and exact original bytes. The official zstd CLI must independently decode each
complete frame to the original before the importer accepts it.

Each case owns a name and borrows immutable encoded and original byte slices.
Frame metadata consists of plain size, flag, and block-type values, which need
no unsafe alternative. Runtime generated cases own their byte buffers in
`Vec<u8>` and use filesystem paths rather than non-owning pointers. Explicit
output and window limits bound Rust decoder allocations. SHA-256 metadata
provides fixture provenance; byte-for-byte assertions establish correctness.

## Work items

- [x] Analyze fixture ownership and decompose the replay module.
- [x] Generate a seed-fixed, version-pinned frame/header corpus with hashes.
- [x] Replay all complete frames through Rust, check resource limits and truncation.
- [x] Add an opt-in configurable long generator run and verify the port.

## Mode mapping

The upstream `test-decodecorpus` target runs randomized self-tests against
libzstd. This port changes the implementation under test to `zstdoxide` and
retains official `generateCorpus` output and its independently maintained
original buffer. The deterministic corpus samples mixed/raw/RLE/compressed
blocks, known and unknown content size, small block/content caps, normal block
caps, and standalone frame headers. It does not claim exhaustive random-space
coverage.

Dictionary APIs are available, but this generator's random dictionary/frame
mode has not yet been adapted. Public raw-block and magicless APIs needed by
`--gen-blocks` and `--no-magic` remain unavailable. Complete generated ordinary
frames also run through fragmented streaming in `tests/upstream_streaming.rs`;
this does not port every original `runFrameTest` branch or fresh-generator
streaming campaign. The generator's own command-line error handling is an
upstream executable test, not Rust codec coverage.

Generated fixtures derive from upstream algorithms, copyright Meta Platforms,
Inc. and affiliates. The upstream BSD license is retained in `LICENSE.zstd`.


## Corpus and replay

The committed suite contains 96 complete frames, exact original buffers, and
16 standalone headers, occupying approximately 1.15 MB including metadata.
The 96 frames contain 180 raw, 111 RLE, and 315 compressed blocks. Seventy-six
frames advertise their content size and twenty omit it. Upstream deliberately
enables content checksums for every generated frame; this corpus does not claim
to cover frames without checksums.

Each row generates twelve frames. `-s` fixes the initial seed; upstream advances
its random state between frames, so an individual frame is identified by both
mode and ordinal, not by treating its ordinal as another seed.

| Mode | Seed | Maximum block size | Maximum content size | Additional options |
| --- | ---: | ---: | ---: | --- |
| mixed | 101 | 128 KiB | 64 KiB | none |
| known-size | 202 | 128 KiB | 64 KiB | `--content-size` |
| raw | 303 | 128 KiB | 64 KiB | `--block-type=0` |
| rle | 404 | 128 KiB | 64 KiB | `--block-type=1 --content-size` |
| compressed | 505 | 128 KiB | 64 KiB | `--block-type=2` |
| small-blocks | 606 | 1 KiB | 64 KiB | none |
| small-content | 707 | 1 KiB | 1 KiB | none |
| large-content | 808 | 128 KiB | 256 KiB | `--content-size` |

Header-only fixtures use independent seeds 1 through 16. The manifest records
all command arguments, source revision and SHA-256, per-buffer SHA-256 hashes,
header fields, and block sizes. The generated Rust table borrows those files;
no C source or compiled generator is stored in the project.

Seven normal tests compare complete outputs, validate frame/header metadata
and generator caps, reject insufficient output/window budgets, reject truncated
headers/frames, detect changed checksums, and concatenate all 96 frames to
check that decoder state resets at frame boundaries. One opt-in test uses a
configured official generator to replay additional random frames through Rust.
Its original bytes come from the generator's independently maintained original
buffer; it does not call C to decode on behalf of Rust.

## Reproduction and extended runs

```sh
# Build outside the project from the pinned checkout.
make -C /path/to/zstd-v1.5.7/tests decodecorpus
python3 scripts/generate_upstream_decodecorpus.py /path/to/zstd-v1.5.7
python3 scripts/generate_upstream_decodecorpus.py /path/to/zstd-v1.5.7 --check
cargo test --test upstream_decodecorpus
ZSTD_DECODECORPUS=/path/to/zstd-v1.5.7/tests/decodecorpus \
    cargo test --release --test upstream_decodecorpus -- --ignored
```

The importer requires the pinned source and an official v1.5.7 `zstd` CLI;
`--zstd` or `ZSTD_BIN` selects that CLI. `--decodecorpus` selects a separately
built official generator. `--check` regenerates and compares every byte without
writing project files. All 96 imported complete frames must first decode
successfully with the independent official CLI and match the original buffers.

The opt-in test defaults to 4,096 additional mixed frames with initial seed 1,
64 KiB content caps, and 128 KiB block caps. Set `ZSTD_DECODECORPUS_CASES` (at
least 2) and `ZSTD_DECODECORPUS_SEED` to select a larger run or reproduce a
failure. Its error identifies the seed and ordinal. Temporary fixture files
are removed when the test exits. The generator must be built from v1.5.7;
upstream's executable has no version-reporting option, so this runtime test
cannot itself authenticate an arbitrary externally supplied executable.

Validation included the seven regular tests in safe and all-feature builds,
Clippy with warnings denied, fixture regeneration checks, and the full default
4,096-frame opt-in release run. These passed without revealing a codec bug.
