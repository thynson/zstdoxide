# Huffman tables and literals sections

Reference: facebook/zstd v1.5.7, `lib/common/entropy_common.c` (`HUF_readStats`),
`lib/decompress/huf_decompress.c` (`HUF_readDTableX1_wksp`,
`HUF_decompress1X1_usingDTable_internal_body`, and
`HUF_decompress4X1_usingDTable_internal_body`), and
`lib/decompress/zstd_decompress_block.c` (`ZSTD_decodeLiteralsBlock`).

## Structures and ownership

- `HuffmanTable` owns a fallibly allocated `Vec<DecodeEntry>` with exactly
  `1 << table_log` initialized entries. Its maximum table logarithm is 11, as
  required by the Zstandard format. The table is immutable after construction.
  Each entry contains one output byte and a positive number of consumed bits.
  Zero-weight symbols have no entries. Symbols are ordered by increasing weight,
  then byte value. Repeated entries cover suffixes of shorter codes.
- Temporary symbol weights occupy an owned `Vec<u8>` with at most 256 symbols.
  An explicit header describes at most 255 weights; the last nonzero weight is
  inferred from a power-of-two completion. Every weight is checked before any
  shift or table offset is calculated. The complete distribution must have at
  least two weight-one symbols and an even weight-one count.
- `DecodeEntry` is a plain value record shared by both implementations. Artificial
  unsafe storage offers no benefit for its two scalar fields.
- Safe table lookup uses checked slice indexing. `unsafe-huffman` uses an
  equivalent checked API with a local raw-pointer load after a runtime bounds
  check. The table retains ownership for the entire load; no pointer escapes and
  immutable access excludes mutation. Other features do not select this path.
- A literals section borrows encoded bytes by checked ranges and returns an
  owned, fallibly allocated byte vector. Four-stream jump offsets are indices,
  never pointers. Output lengths are bounded before allocation. Stream lengths
  and the prescribed ceiling-quarter output split are validated before decode.
- An `Option<HuffmanTable>` owned by the enclosing frame retains the preceding
  compressed-literals table for treeless sections. Raw/RLE sections preserve it;
  a successfully decoded compressed section replaces it. Failed decode leaves
  the existing table unchanged.

## Independently verifiable work items

1. Parse direct/FSE weights, infer and validate the final weight, construct the
   checked safe/unsafe table, and decode a terminated single Huffman bitstream.
2. Decode raw/RLE/compressed/treeless literals headers and one/four streams with
   bounded allocation, table reuse, and malformed-input tests.

Both items use `alloc` and `core` only. The upstream executable remains an
interoperability oracle, never the implementation. Allocation sites are explicit
for future allocator support and do not assume an unstable allocator API.
