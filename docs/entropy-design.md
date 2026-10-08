# Entropy decoder design

Reference: facebook/zstd v1.5.7, `lib/common/bitstream.h`,
`lib/common/entropy_common.c`, `lib/common/fse_decompress.c`, and the format
specification in `doc/zstd_compression_format.md`. The implementation is
independent Rust; algorithms and wire-format constants follow that reference.

## Data structures and ownership

| Rust structure | Upstream counterpart | Ownership and invariants |
| --- | --- | --- |
| `ReverseBits<'a>` | `BIT_DStream_t` | Borrows an immutable byte slice, owns an integer bit cursor. The cursor excludes the mandatory highest set end-marker bit and never exceeds the slice bit length. Reads move toward bit zero and return the selected bit field in its original little-endian significance. No borrowed raw pointer is stored. |
| `ForwardBits<'a>` | Local state in `FSE_readNCount_body` | Borrows an immutable slice and owns an integer bit position. Reads consume least-significant bits first. The position is checked before every read; normalized counts are bounded by a caller-selected symbol and table-log limit. |
| `FseTable` | `FSE_DTable` | Owns a `Vec<FseEntry>` populated after fallible allocation. A table contains exactly `1 << table_log` initialized entries. Symbols fit in `u8`; transitions remain in that table after adding the specified number of input bits. RLE is represented inline to avoid an infallible allocation. |
| `FseEntry` | `FSE_decode_t` | Plain copied value containing a symbol, transition bit count, and base state. Shared between safe and unsafe builds because it has no pointers or memory-access operations. |
| Normalized counts and next-state counters | `normalizedCounter`, `symbolNext` | Stack arrays indexed by symbols, bounded by 256 entries. Counts are either -1, zero, or positive and absolute frequencies sum to the table size. Plain fixed ownership needs no alternate representation. |
| Huffman weight output | `FSE_decompress_usingDTable_generic` output | Owns a fallibly reserved `Vec<u8>` limited to 255 explicit weights. Two integer states alternate, and the terminal state symbols are emitted according to the FSE bitstream termination rule. |

## Safe and unsafe selection

`unsafe-bitstream` selects an alternate immutable byte access path after explicit
range checks. `unsafe-fse` selects unchecked table reads only after a runtime
state check. All other representations and observable error contracts stay the
same. No unsafe functions are necessary: small private safe accessors contain
locally proved unsafe blocks. No raw pointer survives a lookup.

## Work items

1. Implement reverse bit access with marker validation, exact exhaustion,
   cross-byte reads, and identical checked behavior in both feature modes.
2. Parse normalized counts and construct validated FSE transition tables,
   including sparse counts, low-probability symbols, and inline RLE tables.
3. Decode the alternating two-state FSE Huffman-weight stream, with bounded
   output and malformed/truncated input tests.

Each item is reported to the coordinating agent for its own focused checks and
commit. The coordinator owns `TODO.md`, Cargo features, and module registration.
The codec modules use only `core` and `alloc`.
