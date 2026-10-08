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
