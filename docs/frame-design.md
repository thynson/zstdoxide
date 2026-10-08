# Frame and sequence decoder design

The pinned format reference is facebook/zstd v1.5.7,
`doc/zstd_compression_format.md`. Implementation references are
`ZSTD_getFrameHeader_advanced`, `ZSTD_getcBlockSize`, `ZSTD_decodeSequence`,
`ZSTD_execSequence`, and `XXH64`.

| Structure | Safe representation and invariant | Unsafe alternative |
| --- | --- | --- |
| `ByteCursor` | Borrowed slice and integer position at most its length. Validate subtraction-based remaining length before constructing any range. Failed reads do not advance. | `unsafe-byte-cursor`: unchecked slice access after the same bounds validation; pointers never escape. |
| `FrameHeader` / `BlockHeader` | Copied scalar metadata. Reserved bits and block types are validated. Frame content size and window use u64 until checked against platform/caller limits. | Shared value records, with no pointer operations to optimize. |
| `History` | Owns output in a fallibly grown `Vec<u8>`; stores frame start as an index and a window limit. Every match has a nonzero offset within available frame history and the frame window. Expansion is sequential to support overlapping matches. | `unsafe-history`: locally loaded/stored bytes after capacity reservation and the same source/window validation. No pointer survives a reallocation. |
| Sequence table state | Owns three optional FSE tables and three repeat offsets. Each FSE state is an index; every table read is validated by the FSE backend. | `unsafe-fse` selects table access. Repeat offsets are three owned numbers, so share the value representation. |
| XXH64 accumulator | Four wrapping u64 lanes and checked chunks of the caller's immutable output slice. No heap or pointers. | Reuses byte cursor access where applicable; scalar arithmetic stays shared. A separate artificial unsafe hash state is not useful. |

Resource limits apply before allocation and before each output extension. Frame
history never crosses a concatenated-frame boundary. Raw/RLE blocks do not reset
entropy tables or repeat offsets. A new frame resets all three. Block output is
limited to `min(128 KiB, window_size)`, and the declared content size and optional
checksum are verified at the end. Skippable payloads are checked and skipped by
index. Dictionary IDs other than zero produce an explicit unsupported error;
dictionary-less streams with hidden dictionary dependencies still fail bounds or
entropy validation. Streaming and dictionary loading are later modules.

Work items, in order: cursor/header parsing; bounded history; XXH64 checksums;
sequence decoding including repeat offsets; frame integration. Each has its own
focused tests and commit. The active checklist is in `TODO.md`.
