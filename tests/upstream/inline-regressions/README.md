# Inline upstream regression fixtures

## Design and preservation

Preserve the pinned v1.5.7 inline binary regression inputs from `tests/legacy.c`,
`tests/invalidDictionaries.c`, and `tests/fuzzer.c` as immutable byte files.
Fixture manifests own names, source ranges, original literal spellings, byte
lengths, SHA-256 digests, and explicit support classifications. Tests may borrow
bytes; no pointers or unsafe fixture representations are needed.

The importer evaluates only the narrow C string escape and byte-array
syntax used by these pinned declarations. It excludes compiler-added NUL
terminators wherever upstream passes an explicit length or `sizeof(...) - 1`.
It preserves the legacy concatenation and independently verifies its five
component frames with the official CLI. The v0.8 component uses the current
Zstandard frame magic and can be exercised by the Rust decoder independently.

Legacy v0.4 through v0.7 decoders and dictionary construction APIs remain
unsupported. Preserving their fixtures does not mark those test contracts as
passed. Existing skippable regressions have Rust equivalents; exact bytes are
retained for provenance. Plain text input strings and arrays of compression
size expectations are not binary regression fixtures.

Completed work: located all inline binary literals in the three named sources,
extracted exact bytes and source spellings, hashed all fixtures, and verified
the decodable concatenation and all five components with an independent
official decoder. The manifest records active candidates and blocked APIs.

Upstream fixture copyright: Meta Platforms, Inc. and affiliates. The upstream
BSD license is retained in `LICENSE.zstd` at the repository root.


## Fixture mapping

| Fixture | Bytes | Pinned source | Status |
| --- | ---: | --- | --- |
| `legacy-all.zst` | 917 | `legacy.c:181-239`, `COMPRESSED` | Legacy v0.4-v0.7 decoder support required |
| `legacy-all.raw` | 1,195 | `legacy.c:241-260`, `EXPECTED` | Exact expected concatenated output |
| `legacy-v0.4.3.zst` | 198 | Concatenation bytes 0..198 | Legacy decoder required |
| `legacy-v0.5.0.zst` | 184 | Concatenation bytes 198..382 | Legacy decoder required |
| `legacy-v0.6.0.zst` | 178 | Concatenation bytes 382..560 | Legacy decoder required |
| `legacy-v0.7.0.zst` | 178 | Concatenation bytes 560..738 | Legacy decoder required |
| `legacy-v0.8.0.zst` | 179 | Concatenation bytes 738..917 | Current-format decoder can exercise this frame |
| `legacy-frame.raw` | 239 | First fifth of `EXPECTED` | Exact expected output of each component |
| `invalid-repeat-code.dict` | 160 | `invalidDictionaries.c:14-29`, `invalidRepCode` | Dictionary constructor rejection test blocked |
| `fixed-training.dict` | 144 | `fuzzer.c:2646-2657`, `dictBufferFixed` | Dictionary tests blocked |
| `skippable-size-minus-eight.zst` | 8 | `fuzzer.c:732`, `skippable8` | Truncated skippable-frame rejection |
| `skippable-variant-two.zst` | 13 | `fuzzer.c:4114`, `skippableFrame` | Skip five payload bytes; empty output |
| `header-only.dict` | 8 | `fuzzer.c:2912`, inline argument | Dictionary corruption API test blocked |

Byte ranges are half-open. The manifest preserves source literals exactly,
including hexadecimal and octal escapes and the UTF-8 apostrophe in `EXPECTED`.
It also retains byte-array initializer spellings and whole-source SHA-256
hashes. Compiler-added C string terminators are excluded exactly as upstream
callers exclude them. Source line ranges refer to the pinned upstream files,
not this repository.

The v0.8.0 encoder produced the current-format frame magic `28 b5 2f fd`.
Testing that component alone is useful current-format coverage; it does not
complete the legacy suite's concatenated or streaming test contracts.

## Reproduction

```sh
make -C /path/to/zstd-v1.5.7/programs ZSTD_LEGACY_SUPPORT=4 zstd
python3 scripts/import_upstream_regressions.py /path/to/zstd-v1.5.7 \
    --zstd /path/to/zstd-v1.5.7/programs/zstd
python3 scripts/import_upstream_regressions.py /path/to/zstd-v1.5.7 \
    --zstd /path/to/zstd-v1.5.7/programs/zstd --check
```

A v1.5.7 CLI with legacy v0.4 support is required for independent verification;
an executable built with only recent legacy support will reject the first
frame. The importer verifies the pinned Git revision and unchanged source,
compares the official output of the entire concatenation and every component
with exact original bytes, verifies both skippable expectations, and either
writes or compares all thirteen fixtures plus their manifest. Both generation
and `--check` passed with an external official build using legacy support 4.

## Rust replay

`tests/upstream_inline_regressions.rs` has four active tests: byte-exact decoding
of the modern v0.8 component, every truncation of that component and its short
output budget, exact valid skippable bytes and their truncations, and the exact
malformed huge skippable-size regression.

`decodes_all_five_official_legacy_frames` ports the desired successful decode
and byte-equality assertions of `tests/legacy.c::testSimpleAPI`. It is ignored
with an explicit dependency on v0.4-v0.7 decoder support. It has a real test body;
when explicitly run today, it fails with `InvalidMagic`. That known failure
must not be reported as a passing legacy port. The legacy streaming and frame
inspection contracts remain separate pending work.

No dictionary success or constructor-rejection test can call a Rust dictionary
API yet; those preserved inputs remain pending. Decoder rejection of dictionary
bytes as a frame would not test the upstream constructor contract.

```sh
cargo test --test upstream_inline_regressions
# Expected to fail until legacy decoders exist:
cargo test --test upstream_inline_regressions \
    decodes_all_five_official_legacy_frames -- --ignored
```

The four active tests passed with default, all, and no-default features. Relevant
Clippy with `-D warnings` and formatting passed. The ignored test was explicitly
run to confirm its current `InvalidMagic` failure rather than being counted as
implemented support.
