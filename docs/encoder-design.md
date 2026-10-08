# Initial encoder design

The initial encoder writes checksummed single-segment frames with known content
size and blocks of at most 128 KiB. It uses the smallest valid content-size
field. An empty input has one final empty raw block. Each nonempty block first
supports raw bytes or one-byte RLE; a later work item adds actual LZ compression.

Work decomposition, completed with separate coordinator commits:

1. Write frame metadata, raw/RLE blocks, and XXH64 checksum. Check size-field
   boundaries and independent official decoding.
2. Add an initialized position-indexed match table with a checked safe backend
   and separately selectable `unsafe-match-table` backend. Test collision,
   reset, stale position, and overlap behavior.
3. Build greedy sequences and raw literals; invert predefined FSE decode tables
   into encoding transitions; emit reverse sequence streams. Choose a compressed
   block only when its complete payload is smaller than a raw block.
4. Verify both encoder backends against the official decoder across deterministic
   repetitive, mixed, incompressible, and multi-block data.

## Ownership and invariants

`compress` borrows input and owns its output in a `Vec<u8>`. All growth uses
fallible reservation before writes. Output size arithmetic is checked. Frame and
block headers are scalar values and share their representation in all feature
modes. The checksum borrows the original input and is computed once per frame.

`MatchTable` owns initialized integer positions, using `usize::MAX` as the empty
sentinel. Four-byte hashes map to bounded table indices. Matches refer to earlier
positions in the same block and are verified byte by byte. The selected table
backend performs exactly the same insert/lookup decisions; only indexed access
changes under `unsafe-match-table`. No raw pointer is stored across calls or
allocation. Greedy parsing initially resets the table per block; it does not
exploit cross-block matches.

Sequences are plain literal length, match length, and offset records. The encoder
uses explicit offset values rather than repeat-offset optimization. Each record
consumes literals followed by a match, and remaining literals finish the block.
An owned initialized bit writer appends little-endian fields to a byte vector and
adds the terminal one bit. Its inverse reader consumes fields in reverse order.

Predefined FSE distributions are shared with the decoder. For each symbol and
next state, an inverse transition identifies the earlier decode-table state and
the bits that lead to the requested next state. Constructing typed inverse
tables avoids reproducing C's packed encoding-table representation. The last
sequence selects states carrying its symbols without adding nonexistent final
state transitions. All preceding sequences are encoded in reverse order.

Raw fallback bounds compressed-block expansion and preserves interoperability
when a predefined entropy distribution is a poor fit. This encoder is an initial
compression strategy, not a reproduction of upstream compression levels or
ratio tuning. Huffman literals, adaptive entropy distributions, repeat-code
selection, dictionaries, and advanced parsing are later work.
