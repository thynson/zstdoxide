# Frame checksum design

The initial codec computes seed-zero XXH64 over the fully decoded frame output.
The frame checksum is the low 32 bits of that value, encoded little endian. A
one-shot implementation therefore needs no owned state or allocation: it borrows
the output slice, accumulates four wrapping `u64` lanes for 32-byte stripes, then
processes the remaining eight-byte, four-byte, and single-byte pieces.

`src/xxhash.rs` uses safe slice chunks and `from_le_bytes`, so source alignment and
native endianness do not affect results. Arithmetic explicitly wraps even when
release overflow checking is enabled. The scalar accumulator is shared between
safe and unsafe codec builds; it contains no pointer-based access policy to
replace. No separate unsafe hash state is introduced. Future streaming support
will add four lanes, a total byte count, and an indexed 32-byte tail buffer.

The algorithm and constants were implemented from `XXH64`, `XXH64_round`,
`XXH64_mergeRound`, `XXH64_finalize`, and `XXH64_avalanche` in
[facebook/zstd v1.5.7 `lib/common/xxhash.h`](https://github.com/facebook/zstd/blob/v1.5.7/lib/common/xxhash.h),
commit `f8745da6ff1ad1e7bab384bd1f9d742439278e99`. Its notice credits
Yann Collet - Meta Platforms, Inc.; this project uses the BSD licensing option.

Tests include common text vectors and reference outputs independently generated
by compiling that pinned header with `XXH_INLINE_ALL` and calling
`XXH64(bytes, length, 0)`. Pattern byte `i` is `(i * 37 + 11) % 256`; selected
lengths exercise stripe boundaries and all tail processing branches, up to one
MiB. A separate test places the same input at offsets 0 through 15 inside a
larger allocation to check unaligned slices and exclude surrounding bytes.
