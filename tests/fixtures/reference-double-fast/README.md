# Rust-readable direct DoubleFast observations

`cases.bin` projects the 15 frozen tiny recipes and 169 frozen wider recipes,
including the separately retained long-literal refinement. Each source collection
checks both padding observations before this converter keeps one representative.
There are 184 records representing 368 padding observations, with no new C runs.

    python3 scripts/pack_double_fast_parser_fixtures.py --check

`--write` regenerates only this projection. The converter verifies fixed checker
identities before importing them; those checkers independently pin all recipes,
C observations and source provenance. The original observations remain unchanged.
Cargo tests read the binary without Python, C, JSON or compression dependencies.

The little-endian stream starts with `ZDFST02\0` and a u32 case count. Each case
has a u32 byte length followed by these fields in order:

- u16 name length and ASCII name bytes;
- u8 window/long-hash/short-hash/search/minimum-match logs or values, then three
  reserved zero bytes;
- u32 target, prefix length, prefix floor, initial/parser/wire repeat triples,
  trailing-literal length, initial current index, ordinary floor and repeat floor;
- u8 long-length kind and u32 long-length position;
- length-prefixed source bytes and their u64 FNV-1a fingerprint;
- u32-counted long seeds, then u32-counted short seeds;
- length-prefixed stored literals and u32-counted six-u32 sequence records:
  raw literal length, raw match-length base, expanded LL/ML, distance and offBase;
- long-table observation, then short-table observation, each consisting of
  u32-counted bucket/raw-index pairs before parsing, u32-counted bucket/old/new
  changes, and two u64 FNV-1a fingerprints over complete little-endian u32 tables.

Blob lengths and record counts use u32. No pointer address or native padding is
serialized. C raw index zero is invalid; a valid index is source position plus
two. Sparse final changes do not claim intermediate store chronology.

`tests/support/double_fast_oracle.rs` checks structural bounds before slicing or
reservation, rejects non-exhausted records, and preserves the two independent
table dimensions. Separate semantic checks replay every byte, resolve the full
wire repeats independently of the production parser, check raw and expanded
lengths, preserve the raw parser's unchanged third repeat, and reconstruct both
complete tables from ordered seeds and changes. Rejection controls cover malformed
records, metadata, sequences and both table observations. These constructed
direct states do not claim ordinary frame publication or public level support.

Attribution: facebook/zstd v1.5.7, revision
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`, copyright Meta Platforms, Inc. and
affiliates. Observations and hash formulas derive from its DoubleFast implementation.
This project uses the BSD-3-Clause option in `../../../LICENSE.zstd`. See the
original collections' NOTICE files and capture provenance for source details.
