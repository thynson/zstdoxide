# Upstream fuzzer and small-program port

Reference: facebook/zstd v1.5.7, `tests/fuzzer.c`, `programs/datagen.c`,
`tests/roundTripCrash.c`, `tests/longmatch.c`, `tests/invalidDictionaries.c`,
`tests/largeDictionary.c`, and `tests/legacy.c`. Derived algorithms and test
inputs use upstream's BSD-3-Clause licensing option; see the upstream license
preserved as root `LICENSE.zstd`. C implementations are not linked into the Rust codec.

## Design recorded before implementation

The executable port targets `compress`, `decompress`, `decompress_with_options`,
and `FrameHeader::parse`. Rust owns generated noise, compressed frames, and
expected bytes in `Vec<u8>`; slices and integer offsets replace borrowed C
pointers. `FuzzerRng` holds one `u32` and deliberately wraps the same arithmetic
as `FUZ_rand`; the separate data-generator state preserves `RDG_rand`'s XOR
step, which differs from the fuzzer's addition step. `NoiseGenerator` owns its
8,192-byte literal distribution table and a scalar RNG state. Previous match
distance is an integer at most the generated prefix length; overlapping copies
advance one checked byte at a time. All records and access paths are safe Rust.
They are test-only workloads, not performance-sensitive codec structures, so no
artificial unsafe backend is warranted.

Skippable-frame constructors own their bytes and encode bounded payload lengths
as little-endian `u32`. Fuzz cases use explicit output/window limits; malformed
input may produce an error or a bounded successful decode, but cannot panic.
Independent upstream-generated vectors verify the generator itself rather than
comparing it only with a second copy of the Rust implementation.

## Work decomposition

1. Implement and verify the deterministic upstream RNG and noise generator.
2. Port named one-shot compression, header, frame, checksum, truncation, empty,
   concatenation, zeros, sequence-count, and literal-buffer cases.
3. Add bounded deterministic roundtrip/mutation cycles and an explicit longer
   stress entry point; map every upstream fuzzer test label and unsupported API.

The mapping below distinguishes complete behavioral ports from adapted tests
and pending capabilities. An adapted test never proves a context/stream/level
API that does not exist. C pointer ABI checks have no direct Rust equivalent.

## Running the executable port

```sh
cargo test --test upstream_fuzzer
cargo test --test upstream_fuzzer --all-features
cargo test --release --test upstream_fuzzer intensive_fuzzer_one_shot_cycles -- --ignored
ZSTDOXIDE_FUZZ_CYCLES=1000 ZSTDOXIDE_FUZZ_SEED=305419896 cargo test --release --test upstream_fuzzer intensive_fuzzer_one_shot_cycles -- --ignored
```

The normal suite executes 256 deterministic cycles (64 each for seeds 0, 1,
305419896, and 4294967295), with sample logs below 17 and five 256-KiB noise
buffers per seed. The opt-in test uses upstream's 30,000-cycle default,
`maxSampleLog = 22`, and five 8-MiB noise buffers. Invalid/zero cycle settings
fail explicitly. One seed and iteration identify a Rust-port reproduction;
omitting unsupported random parameter choices means Rust iteration numbers do
not identify exactly the same original C fuzzer input. The RNG, data-generator
bytes, profile weights, log-length selection, and noisy-source mutation
algorithm are preserved. Exact vector comparisons against upstream compiled
`programs/datagen.c` cover twenty length/profile pairs. FNV-1a uses offset basis
14695981039346656037 and multiplier 1099511628211, with wrapping 64-bit products;
all vector inputs use seed 305419896 and automatic literal probability.

The generator comparison was independently run against the reference C source
in `/tmp`, without adding a C library/runtime dependency to the project. The
normal suite also retains the original 5-MiB baseline with seed zero, 1,000,000/100-byte zero
regressions, all sequence-count increments up to 131,000 bytes, and the complete
literal-buffer growth schedule up to 5 MiB. Corrupted input tests require
failure where invalidity is certain. Noisy source tests also allow successful
bounded outputs, matching the upstream contract.

## Complete diagnostic-label inventory

This table includes every `test%3`/`longtest%3` label in the pinned fuzzer,
including helper-function labels. Source line numbers refer to v1.5.7. Repeated
labels remain separate rows because they run under different API contracts.

- **ported**: observable case assertions are executable through the Rust API.
- **adapted**: original corpus/assertions run, but unavailable parameter selection
  or API setup changes the case.
- **partial**: only a named subset runs; the rest is explicitly identified.
- **blocked**: required codec capability is absent; no passing stub substitutes.
- **not-applicable**: a C ABI/compiler implementation assertion has no Rust
  counterpart, rather than a missing format feature.

All Rust function names below refer to `tests/upstream_fuzzer.rs`.

| Upstream line | Original diagnostic label | Status | Rust test / missing capability |
| ---: | --- | --- | --- |
| 497 | compressBound | blocked | Public compression/decompression bound or frame scanning/skippable metadata API absent. |
| 519 | decompressBound | blocked | Public compression/decompression bound or frame scanning/skippable metadata API absent. |
| 576 | ZSTD_CCtx_setCParams | blocked | Encoder level/parameter configuration, parameter validation, or reusable CCtx API absent. |
| 592 | Check block splitter doesn't oversplit incompressible data (seed %u) | blocked | Block-splitter, superblock, block-size, or btultra2 encoder strategy controls absent. |
| 670 | ZSTD_getErrorName | not-applicable | C numeric error/version/symbol-ABI contract; Rust uses typed Error and Cargo package version. |
| 675 | ZSTD_getErrorName with wrong value | not-applicable | C numeric error/version/symbol-ABI contract; Rust uses typed Error and Cargo package version. |
| 680 | min compression level | blocked | Encoder level/parameter configuration, parameter validation, or reusable CCtx API absent. |
| 685 | default compression level | blocked | Encoder level/parameter configuration, parameter validation, or reusable CCtx API absent. |
| 691 | ZSTD_versionNumber | not-applicable | C numeric error/version/symbol-ABI contract; Rust uses typed Error and Cargo package version. |
| 702 | ZSTD_adjustCParams | blocked | Encoder level/parameter configuration, parameter validation, or reusable CCtx API absent. |
| 715 | compress %u bytes | adapted | `basic_compression_and_decompression`. Original 5-MiB RDG input and full output comparison; level 1/context selection pending. |
| 723 | size of cctx for level 1 | blocked | Reusable/static contexts, custom allocation, workspace sizing or shrink/estimate APIs absent. |
| 730 | decompress skippable frame -8 size | ported | `skippable_frame_minus_eight_size`. Exact malformed eight-byte input; rejects its overflowing declared payload. |
| 738 | ZSTD_getFrameContentSize test | ported | `frame_content_size`. Declared content-size value via FrameHeader::parse. |
| 744 | ZSTD_getDecompressedSize test | ported | `frame_content_size`. Deprecated size-query behavior expressed through typed header metadata. |
| 750 | ZSTD_findDecompressedSize test | partial | `frame_content_size`. Single-frame size is covered; a general concatenated-size scanning API is absent. |
| 756 | tight ZSTD_decompressBound test | blocked | Public compression/decompression bound or frame scanning/skippable metadata API absent. |
| 763 | ZSTD_decompressBound test with invalid srcSize | blocked | Public compression/decompression bound or frame scanning/skippable metadata API absent. |
| 770 | decompress %u bytes | ported | `basic_compression_and_decompression`. Original 5-MiB one-shot output and length. |
| 775 | decompress %u bytes with Huffman assembly disabled | partial | `basic_compression_and_decompression`. Byte-correct decoding covered; Rust has no assembly-selection parameter. |
| 786 | check decompressed result | ported | `basic_compression_and_decompression`. Every decompressed byte compared. |
| 793 | invalid endDirective | blocked | Streaming end-directive/pledged-source-size API absent. |
| 804 | ZSTD_checkCParams | blocked | Encoder level/parameter configuration, parameter validation, or reusable CCtx API absent. |
| 811 | ZSTD_createDCtx_advanced and ZSTD_sizeof_DCtx | blocked | Reusable/static contexts, custom allocation, workspace sizing or shrink/estimate APIs absent. |
| 820 | misc unaccounted for zstd symbols | not-applicable | C numeric error/version/symbol-ABI contract; Rust uses typed Error and Cargo package version. |
| 861 | decompress with null dict | partial | `basic_compression_and_decompression`. No-dictionary behavior covered; explicit dictionary-context API absent. |
| 873 | decompress with null DDict | partial | `basic_compression_and_decompression`. No-dictionary behavior covered; prepared-dictionary API absent. |
| 885 | decompress with 1 missing byte | ported | `decompress_with_one_missing_byte`. Truncated source rejected with UnexpectedEof. |
| 891 | decompress with 1 too much byte | ported | `decompress_with_one_extra_byte`. One trailing byte rejected with UnexpectedEof. |
| 897 | decompress too large input | ported | `decompress_with_large_trailing_input`. Overlong source rejected; deterministic zero tail replaces uninitialized C capacity. |
| 903 | decompress into NULL buffer | ported | `decompress_into_zero_output_limit`. Explicit zero output limit expresses the C zero-capacity contract. |
| 909 | decompress with corrupted checksum | partial | `decompress_with_corrupted_checksum`. All four checksum-byte corruptions rejected; forceIgnoreChecksum API absent. |
| 948 | ZSTD_decompressBound test with content size missing | blocked | Public compression/decompression bound or frame scanning/skippable metadata API absent. |
| 968 | check DCtx size is reduced after many oversized calls | blocked | Reusable/static contexts, custom allocation, workspace sizing or shrink/estimate APIs absent. |
| 1015 | ZSTD_compressCCtx() doesn't use advanced parameters | blocked | Encoder level/parameter configuration, parameter validation, or reusable CCtx API absent. |
| 1020 | ZSTD_compress_usingDict() doesn't use advanced parameters | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 1025 | ZSTD_compress_usingCDict() doesn't use advanced parameters | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 1030 | ZSTD_compress_advanced() doesn't use advanced parameters | blocked | Encoder level/parameter configuration, parameter validation, or reusable CCtx API absent. |
| 1035 | ZSTD_compress_usingCDict_advanced() doesn't use advanced parameters | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 1044 | maxBlockSize = 2K | blocked | Block-splitter, superblock, block-size, or btultra2 encoder strategy controls absent. |
| 1063 | ldm fill dict out-of-bounds check | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 1086 | testing dict compression with enableLdm and forceMaxWindow | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 1113 | testing dict compression for determinism | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 1157 | LDM + opt parser with small uncompressible block | blocked | Long-distance matching and optimal parser configuration/size comparison absent. |
| 1199 | testing ldm dictionary gets invalidated | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 1277 | testing refPrefx vs refPrefx + ldm (size comparison) | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 1344 | in-place decompression | blocked | Caller-owned overlapping input/output and decompression-margin API absent. |
| 1369 | in-place decompression with 2 frames | blocked | Caller-owned overlapping input/output and decompression-margin API absent. |
| 1398 | Check block splitter with 64K literal length | blocked | Block-splitter, superblock, block-size, or btultra2 encoder strategy controls absent. |
| 1467 | superblock uncompressible data: too many nocompress superblocks | blocked | Block-splitter, superblock, block-size, or btultra2 encoder strategy controls absent. |
| 1516 | superblock with no literals | blocked | Block-splitter, superblock, block-size, or btultra2 encoder strategy controls absent. |
| 1542 | superblock enough room for checksum | blocked | Block-splitter, superblock, block-size, or btultra2 encoder strategy controls absent. |
| 1554 | compress a NULL input with each level | partial | `compress_and_decompress_empty_frames`. Empty input encoded; every-level parameter sweep absent. |
| 1567 | check CCtx size after compressing empty input | blocked | Reusable/static contexts, custom allocation, workspace sizing or shrink/estimate APIs absent. |
| 1577 | decompress empty frame into NULL | partial | `compress_and_decompress_empty_frames`. Known/unknown-size empty one-shot frames; streaming subcase absent. |
| 1620 | reuse CCtx with expanding block size | blocked | Reusable encoder context/reset state and associated parameter controls absent. |
| 1637 | re-using a CCtx should compress the same | blocked | Reusable encoder context/reset state and associated parameter controls absent. |
| 1681 | btultra2 & 1st block | blocked | Block-splitter, superblock, block-size, or btultra2 encoder strategy controls absent. |
| 1721 | ZSTD_CCtx_getParameter() | blocked | Encoder level/parameter configuration, parameter validation, or reusable CCtx API absent. |
| 1764 | ZSTD_CCtx_setCParams() | blocked | Encoder level/parameter configuration, parameter validation, or reusable CCtx API absent. |
| 1808 | ZSTD_CCtx_setFParams() | blocked | Encoder level/parameter configuration, parameter validation, or reusable CCtx API absent. |
| 1832 | ZSTD_CCtx_setParams() | blocked | Encoder level/parameter configuration, parameter validation, or reusable CCtx API absent. |
| 1891 | ldm conditionally enabled by default doesn't change cctx params | blocked | Long-distance matching and optimal parser configuration/size comparison absent. |
| 1912 | overflow protection with large windowLog | blocked | Encoder level/parameter configuration, parameter validation, or reusable CCtx API absent. |
| 1929 | size down context | blocked | Reusable/static contexts, custom allocation, workspace sizing or shrink/estimate APIs absent. |
| 1964 | create static CCtx for level %u | blocked | Reusable/static contexts, custom allocation, workspace sizing or shrink/estimate APIs absent. |
| 1988 | compress small input with small static CCtx | blocked | Reusable/static contexts, custom allocation, workspace sizing or shrink/estimate APIs absent. |
| 1995 | compress large input with small static CCtx (must fail) | blocked | Reusable/static contexts, custom allocation, workspace sizing or shrink/estimate APIs absent. |
| 2003 | resize context to full CCtx size | blocked | Reusable/static contexts, custom allocation, workspace sizing or shrink/estimate APIs absent. |
| 2009 | compress large input with static CCtx | blocked | Reusable/static contexts, custom allocation, workspace sizing or shrink/estimate APIs absent. |
| 2016 | compress small input often enough to trigger context reduce | blocked | Reusable/static contexts, custom allocation, workspace sizing or shrink/estimate APIs absent. |
| 2028 | init CCtx for level %u | blocked | Reusable/static contexts, custom allocation, workspace sizing or shrink/estimate APIs absent. |
| 2032 | compression again with static CCtx | blocked | Reusable/static contexts, custom allocation, workspace sizing or shrink/estimate APIs absent. |
| 2039 | simple decompression test with static DCtx | blocked | Reusable/static contexts, custom allocation, workspace sizing or shrink/estimate APIs absent. |
| 2046 | check decompressed result | blocked | Reusable/static contexts, custom allocation, workspace sizing or shrink/estimate APIs absent. |
| 2050 | init CCtx for too large level (must fail) | blocked | Reusable/static contexts, custom allocation, workspace sizing or shrink/estimate APIs absent. |
| 2055 | init CCtx for small level %u (should work again) | blocked | Reusable/static contexts, custom allocation, workspace sizing or shrink/estimate APIs absent. |
| 2059 | use CStream on CCtx-sized static context (should fail) | blocked | Reusable/static contexts, custom allocation, workspace sizing or shrink/estimate APIs absent. |
| 2068 | resize context to CStream size, then stream compress | blocked | Reusable/static contexts, custom allocation, workspace sizing or shrink/estimate APIs absent. |
| 2078 | CStream for small level %u | blocked | Reusable/static contexts, custom allocation, workspace sizing or shrink/estimate APIs absent. |
| 2086 | init static CStream with dictionary (should fail) | blocked | Reusable/static contexts, custom allocation, workspace sizing or shrink/estimate APIs absent. |
| 2091 | use DStream on DCtx-sized static context (should fail) | blocked | Reusable/static contexts, custom allocation, workspace sizing or shrink/estimate APIs absent. |
| 2100 | test estimation functions with default cctx params | blocked | Reusable/static contexts, custom allocation, workspace sizing or shrink/estimate APIs absent. |
| 2142 | test estimation functions with maxBlockSize = 0 | blocked | Reusable/static contexts, custom allocation, workspace sizing or shrink/estimate APIs absent. |
| 2192 | Static context sizes for negative levels | blocked | Reusable/static contexts, custom allocation, workspace sizing or shrink/estimate APIs absent. |
| 2205 | create ZSTDMT CCtx | blocked | Multithreaded compressor and shared thread-pool API absent. |
| 2216 | compress %u bytes with 2 threads | blocked | Multithreaded compressor and shared thread-pool API absent. |
| 2222 | decompressed size test | blocked | Multithreaded compressor and shared thread-pool API absent. |
| 2230 | decompress %u bytes | blocked | Multithreaded compressor and shared thread-pool API absent. |
| 2235 | check decompressed result | blocked | Multithreaded compressor and shared thread-pool API absent. |
| 2242 | compress -T2 with checksum | blocked | Multithreaded compressor and shared thread-pool API absent. |
| 2251 | decompress %u bytes | blocked | Multithreaded compressor and shared thread-pool API absent. |
| 2259 | compress empty string and decompress with small window log | blocked | Streaming decoder with configurable window-log/reset context absent. |
| 2281 | compress with block splitting | blocked | Block-splitter, superblock, block-size, or btultra2 encoder strategy controls absent. |
| 2290 | compress -T2 with/without literals compression | blocked | Multithreaded compressor and shared thread-pool API absent. |
| 2305 | Multithreaded ZSTD_compress2() with rsyncable | blocked | Multithreaded compressor and shared thread-pool API absent. |
| 2318 | setting multithreaded parameters | blocked | Multithreaded compressor and shared thread-pool API absent. |
| 2345 | compress multiple frames | adapted | `decompress_multiple_frames_with_skippable_frame`. Original segment sizes/order and 129-KiB skip; compression level selection absent. |
| 2376 | get decompressed size of multiple frames | blocked | Public compression/decompression bound or frame scanning/skippable metadata API absent. |
| 2381 | get tight decompressed bound of multiple frames | blocked | Public compression/decompression bound or frame scanning/skippable metadata API absent. |
| 2386 | decompress multiple frames | ported | `decompress_multiple_frames_with_skippable_frame`. Whole concatenation decoded and combined output limit checked. |
| 2391 | check decompressed result | ported | `decompress_multiple_frames_with_skippable_frame`. Every byte in the concatenated output compared. |
| 2396 | read/write a skippable frame | partial | `skippable_frames_with_all_magic_variants`. All sixteen variants decoded; library write/extract APIs absent. |
| 2420 | read/write an empty skippable frame | partial | `skippable_frames_with_all_magic_variants`. All sixteen empty variants decoded; library write/extract APIs absent. |
| 2443 | copy context too soon | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 2448 | load dictionary into context | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 2453 | compress with flat dictionary | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 2461 | frame built with flat dictionary should be decompressible | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 2469 | compress with duplicated context | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 2480 | frame built with duplicated context should be decompressible | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 2488 | decompress with DDict | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 2496 | decompress with static DDict | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 2508 | check content size on duplicated context | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 2554 | flat-dictionary efficiency test | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 2665 | dictBuilder on cyclic data | blocked | Dictionary training/COVER/FASTCOVER and dictionary compression APIs absent. |
| 2675 | dictBuilder | blocked | Dictionary training/COVER/FASTCOVER and dictionary compression APIs absent. |
| 2682 | Multithreaded COVER dictBuilder | blocked | Dictionary training/COVER/FASTCOVER and dictionary compression APIs absent. |
| 2696 | COVER dictBuilder with shrinkDict | blocked | Dictionary training/COVER/FASTCOVER and dictionary compression APIs absent. |
| 2712 | Multithreaded FASTCOVER dictBuilder | blocked | Dictionary training/COVER/FASTCOVER and dictionary compression APIs absent. |
| 2726 | FASTCOVER dictBuilder with shrinkDict | blocked | Dictionary training/COVER/FASTCOVER and dictionary compression APIs absent. |
| 2742 | check dictID | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 2747 | check dict header size no error | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 2752 | check dict header size correctness | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 2758 | compress with dictionary | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 2765 | retrieve dictID from dictionary | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 2771 | retrieve dictID from frame | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 2777 | frame built with dictionary should be decompressible | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 2788 | estimate CDict size | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 2794 | compress with CDict | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 2809 | retrieve dictID from frame | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 2815 | frame built with dictionary should be decompressible | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 2826 | compress with static CDict | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 2854 | ZSTD_compress_usingCDict_advanced, no contentSize, no dictID | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 2868 | try retrieving contentSize from frame | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 2874 | frame built without dictID should be decompressible | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 2886 | ZSTD_compress_advanced, no dictID | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 2896 | frame built without dictID should be decompressible | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 2907 | dictionary containing only header should return error | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 2920 | bufferless api with cdict | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 2941 | Building cdict w/ ZSTD_dct_fullDict on a good dictionary | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 2949 | Building cdict w/ ZSTD_dct_fullDict on a rawContent (must fail) | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 2963 | Loading rawContent starting with dict header w/ ZSTD_dct_auto should fail | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 2976 | Loading rawContent starting with dict header w/ ZSTD_dct_rawContent should pass | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 2987 | Testing non-attached CDict with ZSTD_dct_rawContent | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 3001 | ZSTD_CCtx_refCDict() then set parameters | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 3013 | Loading dictionary before setting parameters is the same as loading after | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 3032 | Loading a dictionary clears the prefix | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 3040 | Loading a dictionary clears the cdict | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 3050 | Loading a cdict clears the prefix | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 3060 | Loading a cdict clears the dictionary | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 3070 | Loading a prefix clears the dictionary | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 3078 | Loading a prefix clears the cdict | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 3088 | Loaded dictionary persists across reset session | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 3104 | Loaded dictionary is cleared after resetting parameters | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 3124 | ZSTD_decompressDCtx() with dictionary | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 3146 | ZSTD_decompressDCtx() with ddict | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 3166 | ZSTD_decompressDCtx() with prefix | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 3181 | ZSTD_fast attach dictionary with hashLog = 25 and chainLog = 25 | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 3208 | ZSTD_dfast attach dictionary with hashLog = 25 and chainLog = 25 | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 3235 | ZSTD_lazy attach dictionary with hashLog = 29 and searchLog = 4 | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 3265 | Dictionary with non-default repcodes | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 3312 | ZSTD_decompressDCtx() with multiple ddicts | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 3404 | ZDICT_trainFromBuffer_cover | blocked | Dictionary training/COVER/FASTCOVER and dictionary compression APIs absent. |
| 3415 | check dictID | blocked | Dictionary training/COVER/FASTCOVER and dictionary compression APIs absent. |
| 3420 | ZDICT_optimizeTrainFromBuffer_cover | blocked | Dictionary training/COVER/FASTCOVER and dictionary compression APIs absent. |
| 3429 | check dictID | blocked | Dictionary training/COVER/FASTCOVER and dictionary compression APIs absent. |
| 3440 | Check input length for magic number | ported | `check_input_length_for_magic_number`. Every source length below four is rejected. |
| 3446 | Check magic Number | ported | `check_magic_number`. Original corrupted first magic byte rejected. |
| 3453 | Content size verification | blocked | Streaming end-directive/pledged-source-size API absent. |
| 3469 | negative compression level | blocked | Encoder level/parameter configuration, parameter validation, or reusable CCtx API absent. |
| 3496 | parameters in order | blocked | Encoder level/parameter configuration, parameter validation, or reusable CCtx API absent. |
| 3513 | parameters disordered | blocked | Encoder level/parameter configuration, parameter validation, or reusable CCtx API absent. |
| 3533 | get dParameter bounds | blocked | Decoder parameter bound/query/reset API absent. |
| 3539 | wrong dParameter | blocked | Decoder parameter bound/query/reset API absent. |
| 3548 | out of bound dParameter | blocked | Decoder parameter bound/query/reset API absent. |
| 3568 | magic-less format test | blocked | Magicless encoding/decoding and its format-selection API absent. |
| 3579 | decompress normally (should fail) | blocked | Magicless encoding/decoding and its format-selection API absent. |
| 3585 | decompress of magic-less frame | blocked | Magicless encoding/decoding and its format-selection API absent. |
| 3608 | empty magic-less format test | blocked | Magicless encoding/decoding and its format-selection API absent. |
| 3619 | decompress of empty magic-less frame | blocked | Magicless encoding/decoding and its format-selection API absent. |
| 3641 | Decompression parameter reset test | blocked | Decoder parameter bound/query/reset API absent. |
| 3678 | Block compression test | blocked | Public raw-block/bufferless streaming compression and decompression API absent. |
| 3684 | Block decompression test | blocked | Public raw-block/bufferless streaming compression and decompression API absent. |
| 3691 | Huge block streaming compression test | blocked | Public raw-block/bufferless streaming compression and decompression API absent. |
| 3705 | Dictionary Block compression test | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 3716 | Dictionary Block decompression test | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 3733 | Block compression with CDict | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 3749 | RLE detection test | ported | `rle_detection_workload`. Original B/A runs, roundtrip, and <=46-byte compressed-size bound retained. |
| 3764 | ZSTD_generateSequences decode from sequences test | blocked | Public sequence generation/external-sequence compression API absent. |
| 3797 | ZSTD_generateSequences too small output buffer | blocked | Public sequence generation/external-sequence compression API absent. |
| 3822 | ZSTD_getSequences followed by ZSTD_compressSequences | blocked | Public sequence generation/external-sequence compression API absent. |
| 3881 | ZSTD_compressSequencesAndLiterals | blocked | Public sequence generation/external-sequence compression API absent. |
| 3985 | compress %u zeroes | ported | `multiple_blocks_of_zeroes`. Original 1,000,000 zero bytes compressed. |
| 3990 | decompress %u zeroes | ported | `multiple_blocks_of_zeroes`. Original 1,000,000-byte output compared. |
| 3997 | compress %u zeroes | ported | `all_zeroes_issue_137`. Original 100 zero bytes compressed. |
| 4002 | decompress %u zeroes | ported | `all_zeroes_issue_137`. Original 100-byte output compared. |
| 4032 | growing nbSeq | partial | `growing_sequence_count`. Original counts and triple corpus retained; context reuse and level 19 absent. |
| 4050 | compress lots 3-bytes sequences | adapted | `many_three_byte_sequences`. Exact 131,000-byte seeded triple corpus; level 19 selection absent. |
| 4055 | decompress lots 3-bytes sequence | ported | `many_three_byte_sequences`. Complete triple-corpus output comparison. |
| 4061 | growing literals buffer | partial | `growing_literals_buffer`. Original literal profile and full 1..5-MiB schedule; context reuse absent. |
| 4079 | incompressible data and ill suited dictionary | blocked | Dictionary training/COVER/FASTCOVER and dictionary compression APIs absent. |
| 4117 | ZSTD_findFrameCompressedSize on skippable frame | blocked | Public compression/decompression bound or frame scanning/skippable metadata API absent. |
| 4121 | ZSTD_getFrameContentSize on skippable frame | blocked | Public compression/decompression bound or frame scanning/skippable metadata API absent. |
| 4125 | ZSTD_getFrameHeader on skippable frame | blocked | Public compression/decompression bound or frame scanning/skippable metadata API absent. |
| 4140 | testing ZSTD error code strings | not-applicable | C numeric error/version/symbol-ABI contract; Rust uses typed Error and Cargo package version. |
| 4149 | testing ZSTD dictionary sizes | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 4172 | testing FSE_normalizeCount() PR#1255 | blocked | FSE normalized-count encoding/normalization API not implemented; decoder alone does not cover it. |
| 4190 | testing FSE_writeNCount() PR#2779 | blocked | FSE normalized-count encoding/normalization API not implemented; decoder alone does not cover it. |
| 4206 | testing bitwise intrinsics PR#3045 | not-applicable | C compiler-intrinsic implementation regression; Rust uses language integer operations. |
| 4275 | passing wrong full dict should fail on compressStream2 refPrefix | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 4305 | small dictionary with multithreading and LDM | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 4332 | ZSTD_getCParams() + dictionary | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 4346 | ZSTD_adjustCParams() + dictionary | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |
| 4357 | check compression mem usage monotonicity over levels for estimateCCtxSize() | blocked | Reusable/static contexts, custom allocation, workspace sizing or shrink/estimate APIs absent. |
| 4373 | check estimateCCtxSize() always larger or equal to ZSTD_estimateCCtxSize_usingCParams() | blocked | Reusable/static contexts, custom allocation, workspace sizing or shrink/estimate APIs absent. |
| 4395 | thread pool API tests | blocked | Multithreaded compressor and shared thread-pool API absent. |
| 4438 | table cleanliness through index reduction | blocked | Persistent streaming match-table index reduction and static context APIs absent. |
| 4507 | testing ldm no regressions in size for opt parser | blocked | Long-distance matching and optimal parser configuration/size comparison absent. |
| 4544 | testing cdict compression with different attachment strategies | blocked | Raw/prepared dictionaries, prefixes, dictionary attachment/reset, or dictionary parameter APIs absent. |

## Fuzzer groups outside the numbered diagnostics

| Upstream function / group | Status and retained obligation |
| --- | --- |
| `FUZ_rand`, `FUZ_randomLength`, `FUZ_rLogLength` | Ported scalar RNG/logarithmic lengths in `support`; independently verified RNG vectors. |
| `RDG_genBuffer`, `RDG_genBlock`, literal distribution | Ported safe owned generator; exact upstream output vectors. Empty Rust input is explicitly handled safely. |
| `FUZ_bug976` | C preprocessor/MIN-macro compile-time regression; no corresponding Rust macro dependency. |
| `FUZ_mallocTests_internal`, `FUZ_mallocTests` | Blocked: custom allocator/context APIs, allocation counters, and macOS malloc-size behavior. |
| `FUZ_decodeSequences` | Blocked: public sequence-export/import test API absent. |
| `threadPoolTests`, `compress_thread` | Blocked: shared thread-pool creation/reuse/simultaneous compression. |
| `fuzzerTests`: simple compression/header/decompression | Adapted in `deterministic_fuzzer_one_shot_cycles`; compression levels and reusable context selection absent. |
| `fuzzerTests`: too-small compression destination | Blocked: compressor returns an owned vector; caller-owned destination API absent. |
| `fuzzerTests`: truncated source / too-small decoded destination | Ported via exact truncation/explicit output-limit rejection; safe slice ownership replaces C canaries. |
| `fuzzerTests`: noisy source | Ported bounded mutation/no-panic contract with the upstream mutation algorithm. |
| `fuzzerTests`: bufferless/scattered-segment dictionary compression, decompression and noise | Blocked: streaming contexts, dictionaries, context copying, chunk lifetime/reuse, and stream-state APIs absent. |
| `fuzzerTests`: `ZSTD_getFrameHeader`, decompressed-size scan | Header content-size assertion ported; general concatenation-size scanner absent. |
| `findDiff`, display/CLI/clock and seed parsing | Rust assertion messages, test harness, and explicit deterministic seed/cycle controls replace process-exit and logging glue. |

## Small upstream programs

| Source / entry point | Status and executable mapping |
| --- | --- |
| `roundTripCrash.c::roundTripTest`, `roundTripCheck`, `checkBuffers` | Adapted in `round_trip_crash_corpora`: exact length/byte equality across fifty deterministic corpora. XXH32-derived level selection is pending encoder levels. |
| `roundTripCrash.c::cctxParamRoundTripTest` | Blocked: reusable parameter context, streaming, two-worker compression and overlap controls. |
| `roundTripCrash.c::loadFile`, `fileCheck`, `main` | C command-line/file diagnostics are test-driver glue; Rust integration harness supplies deterministic slices. No user-file reader is claimed. |
| `longmatch.c::main`, `compress` | Only its 256-KiB source layout is adapted in `long_match_input_layout`. Continuous 8-GiB streaming, randomized flush boundaries, level/strategy controls, and index rollover remain blocked. A smaller one-shot test is not an index-overflow port. |
| `invalidDictionaries.c::main` | Blocked: must reject its embedded invalid-repcode dictionary in both prepared compression and decompression dictionary constructors. Rejecting it as a frame would not port this test. |
| `largeDictionary.c::main`, `compress` | Blocked: original 2-GiB buffer, 1-GiB chunks, 31-bit window, worker/optimal-parser configuration and continuous stream history. The code executes ten continued chunks plus one final chunk; its comment says 30 GB. |
| `legacy.c::testSimpleAPI` | Blocked: successful decoding of the embedded historical format versions is required; rejecting their magic does not satisfy the test. |
| `legacy.c::testStreamingAPI` | Blocked: historical frame decoding and streaming progress/reset contracts. |
| `legacy.c::testFrameDecoding` | Blocked: historical frame boundary scans and decompression bounds. |

This is a complete inventory of these sources, not a claim that all upstream
capabilities or tests are already executable in the Rust codec.
