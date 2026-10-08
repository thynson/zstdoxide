# Independent entropy vectors

The upstream tests `tests/fuzz/fse_read_ncount.c`, `huf_decompress.c`, and
`huf_round_trip.c` motivate these decoder assertions. The reference is zstd
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
consumed. Rust verifies these outputs with and without trailing bytes.

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
