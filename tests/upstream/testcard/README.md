# Official single-file examples and kernel empty-frame fixture

## Design and attribution

Fixtures are immutable borrowed byte slices; decoder/encoder results own
`Vec<u8>` buffers. Exact source bytes and SHA-256 hashes identify provenance.
The importer parses only comma-separated byte literals in pinned upstream
sources and uses the official CLI as an independent fixture verifier.

Before implementation the work is decomposed into exact fixture import,
byte-equality decoder and encoder assertions, explicit resource-boundary tests,
and safe/feature/no_std verification. No unsafe test data structure is needed.

`testcard.dxt1` and `testcard.zst` derive from the single-file examples by
**Carl Woffenden, Numfum GmbH**, which upstream explicitly releases under a
**CC0 license**. See `NOTICE` in this directory and the source paths in the
manifest. The Linux kernel empty-frame fixture is copyright Meta Platforms,
Inc. and affiliates, retained under the upstream BSD license in `LICENSE.zstd`.

The Rust tests port the one-shot data contracts of `simple.c`, `roundtrip.c`,
and the empty-frame assertion in `contrib/linux-kernel/test/static_test.c`.
Compression at `ZSTD_maxCLevel()`, C amalgamation, WebAssembly/WebGL rendering,
and Linux kernel workspace/context APIs remain unavailable or inapplicable;
a Rust round-trip does not claim to test those interfaces or compression levels.

## Reproduction and validation

```sh
python3 scripts/import_upstream_testcard.py /path/to/zstd-v1.5.7
python3 scripts/import_upstream_testcard.py /path/to/zstd-v1.5.7 --check
cargo test --test upstream_testcard
```

The pinned importer verifies unchanged source and exact official CLI output.
The five Rust tests check fixture decoding, Rust round-trip, truncated data,
output limits, and the checksum of the empty frame. All five passed with safe,
all-feature, and no-default-feature builds; relevant Clippy and formatting also
passed. Fixture regeneration with `--check` passed.
