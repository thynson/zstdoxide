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
compressor. The Module 8 encoder tests now reproduce all thirty one-stream and
all thirty four-stream payloads byte for byte using those exact descriptions.
The complete `huf_round_trip.c` parameter driver remains partial; fixed vectors
do not replace its randomized capacities, table depths or source sizes.

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

## Huffman stream encoding and independent table verification

`tests/upstream/huffman_encode.rs` verifies the encoder against the existing
`huffman.bin` collection without changing its bytes or provenance. Its separate
live C oracle, `scripts/oracles/huffman_streams.c`, receives the exact table
description, original source and Rust stream. The pinned C library checks
weights/code lengths, re-encodes that table byte for byte, and decodes with its
X1 and X2 engines. It never supplies compressed output to the production Rust
implementation.

Both safe and all-feature builds passed these three explicit C checks:

- 85 same-table checks: thirty Rust 1X and thirty stored C 4X payloads, fifteen
  two-symbol boundaries, and ten prepared HTTP log-12 cases.
- 55 sparse-table checks: eleven descriptions across five source lengths,
  covering direct/FSE weights, missing symbols, maximum symbols 128/129/255,
  skewed trees and the complete-alphabet serialization fallback.
- 59 Rust 4X checks: thirty official tables, tiny and size-field boundaries,
  prepared log-12 cases and the 196,618-byte maximum payload from a 128 KiB
  source of twelve-bit symbols. Each first-quarter stream occupies 49,153 bytes.

These counts describe oracle invocations, not distinct upstream test groups.
At source sizes six and nine, the independent oracle assembles four C 1X
streams because the C 4X entrypoint returns zero below twelve source bytes.
The empty 1X case checks its `01` marker without claiming a successful C
zero-output decode. An X2 table-log fallback is explicit in the oracle;
none of these successful runs required it.

```sh
cargo test --lib upstream_huffman_encode_tests
ZSTD_UPSTREAM=/path/to/zstd-v1.5.7 \
  cargo test --all-features --lib upstream_huffman_encode_tests -- --ignored --nocapture
```

The public `tests/entropy_encoding.rs` suite adds eight ordinary tests and two
explicit C CLI tests. Its independent frame inspector proves a literal-only
zero-sequence block, 1X/4X field transitions, inline/treeless modes, preservation
across raw/RLE blocks, dictionary replacement, reset/prefix isolation and
identical fragmented output. Each safe/all-feature C run decoded twelve whole
frames. These format witnesses supplement the original entropy targets; they
do not complete upstream compression-level, output-buffer or strategy APIs.

The exact outstanding selector, standalone table-log, capacity and corpus work
is decomposed in [the Module 8 coverage plan](../../../docs/upstream-entropy-plan.md).
