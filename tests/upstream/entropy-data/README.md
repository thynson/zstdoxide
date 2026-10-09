# Independent entropy vectors

The upstream tests `tests/fuzz/fse_read_ncount.c`, `huf_decompress.c`, and
`huf_round_trip.c` motivate these reader/writer assertions. The reference is zstd
v1.5.7, revision `f8745da6ff1ad1e7bab384bd1f9d742439278e99`.

The work is decomposed before implementation into independent oracle fixture
generation, exact normalized-count parser assertions, valid Huffman decoder
replay, and bounded arbitrary-input entropy checks. The production crate does
not link to C. `scripts/oracles/entropy_vectors.c` is an offline fixture tool
compiled against an external pinned source checkout. Its scalar generators,
fixed arrays, and binary records never enter the Rust library. Rust tests own
vectors and use slice indices; metadata records need no unsafe alternative.

The FSE corpus covers table logarithms 5 through 12, eight maximum symbols,
and eight distributions including single symbols, long zero runs, rare
symbols, uniform counts, and deterministic random counts. Every header is
written and read back by upstream, recording all frequencies and exact bytes
consumed. Rust verifies these outputs with and without trailing bytes, and its
normalized-count writer now reproduces all 512 complete headers byte for byte.

Huffman vectors contain upstream-written tables, one- and four-stream payloads,
and original bytes. They test the Rust decoder independently of the Rust
compressor. Upstream's entropy encoders, separate X1/X2 implementations, CPU
dispatch flags, workspaces, and their full randomized roundtrips remain
unported. Fixed vectors are finite coverage, not a complete fuzz campaign.

`manifest.json` records hashes of the vectors, generator, and pinned reference
sources. Run `python3 scripts/generate_upstream_entropy.py /path/to/zstd`
after building that checkout's `lib/libzstd.a`; append `--check` to reproduce
and verify bytes without changing files. Derived fixture data uses the BSD
license option recorded in root `LICENSE.zstd`.

## External Huffman input replay

The opt-in `replays_official_huffman_decoder_corpus` test consumes parameters
from the input tail in the order used by `huf_decompress.c`. It exercises both
CPU-flag byte-consumption paths, then decodes the original remaining input as
one or four streams. It deliberately preserves the upstream target's use of
the whole remaining payload, including table bytes, as the stream input.
Input files are limited to 1 MiB and output to `8 * input_size + 500` bytes.
Rust still uses its own table representation and Zstandard table-log cap 11;
upstream's independent X1/X2 engines, CPU flags, caller workspace, and maximum
table-log setting are not implemented by this adaptation.

```sh
ZSTDOXIDE_HUF_CORPUS=/external/cache/extracted/huf_decompress \
    cargo test --release --all-features --lib \
    replays_official_huffman_decoder_corpus -- --ignored --nocapture
```

All 605 files in the separately hashed 2026-10-08 rolling corpus snapshot
passed with both parameter-consumption paths; 604 paths produced valid tables.
That snapshot is not part of the pinned v1.5.7 Git tree. The corpus fetcher and
`tests/upstream/external-corpora.json` preserve its separate provenance.

## Complete normalized-count target

The `fse_read_ncount.c:28-100` target is ported in `tests/upstream/entropy.rs`.
Its shared tail producer selects table logs 5 through 12, maximum symbols 0
through 255, signed normalized counts, the optional final -1 representation,
random suffix size and every suffix byte in original order. The driver retains
the 512-byte write-bound assertion and verifies exact header consumption,
maximum symbol, table log and counts after readback. It executes 10,241
deterministic inputs, including every log/maximum-symbol combination under
three tail patterns, empty input and 4,096 generated inputs.

Both safe and all-feature release builds replayed the entire independently
pinned 79-file corpus (908 source bytes), without skipping or truncating files.
The external runner admits at most 65,536 regular files, 1 MiB per file and
64 MiB overall. Generated header-plus-suffix buffers remain at most 512 bytes.

```sh
ZSTDOXIDE_FSE_CORPUS=/tmp/zstdoxide-external-corpora/extracted/fse_read_ncount \
  cargo test --release --lib \
  upstream_entropy_tests::replays_official_normalized_count_corpus -- --ignored --nocapture
ZSTDOXIDE_FSE_CORPUS=/tmp/zstdoxide-external-corpora/extracted/fse_read_ncount \
  cargo test --release --all-features --lib \
  upstream_entropy_tests::replays_official_normalized_count_corpus -- --ignored --nocapture
```

All ten ordinary entropy tests passed in each of twelve focused configurations:
safe, each of nine unsafe flags independently, all flags, and no-default-features.
The std configurations retain two explicitly ignored external corpus runners;
the no_std test configuration omits those runners. Strict Clippy and formatting
also passed. Exact commands, individual result summaries and all log SHA-256
hashes are preserved in the
[2026-10-09 evidence record](../../../docs/evidence/2026-10-09-fse-target.json).
The source/case status is complete for this normalized-count target; it does
not assert a port of unrelated FSE or Huffman compression APIs.
