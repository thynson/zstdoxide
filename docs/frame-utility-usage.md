# Compression bound and skippable writing

`compression_bound(source_size)` returns a checked one-pass size estimate using
the pinned v1.5.7 formula. It covers ordinary one-shot compression, dictionary
variants and currently admitted reference one-shot profiles. It excludes arbitrary
streaming flush schedules and separately compressed frame concatenations. It does
not measure scratch/workspace or guarantee allocation succeeds. Values at or above
the width-specific maximum return ResourceLimit. A very large typed numeric bound
can overlap the native C error range; Rust has no reserved error sentinel values.

`write_skippable_frame(output, payload, variant)` writes an eight-byte header and
exact payload into a borrowed mutable slice, returning their combined length.
Variants 0..15 select magic 0x184d2a50..0x184d2a5f. Payload and output are independent
borrows; safe Rust rejects aliasing. No allocation or retained reference is added.

Invalid variant is checked first and returns InvalidParameter. Unrepresentable
u32 length, usize extent overflow or insufficient capacity returns ResourceLimit.
All errors leave the entire destination unchanged. On success bytes after the
returned extent are unchanged. This precedence deliberately differs from native
capacity-first checks when both arguments are invalid. The resulting frame works
with frame metadata, borrowed skippable payload and ordinary concatenated decoding.
See `frame-utility-validation.md` for exact validation and native differences.
