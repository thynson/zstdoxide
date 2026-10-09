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

## Module 8 entropy evidence and remaining strategy assertions

The two exact normalization regressions retain their distinct contracts:
`fuzzer.c:4172` requires the original unit-count call not to divide by zero;
`:4190` has independently verified sparse-header bytes, while its exact
caller-owned nine-byte destination behavior remains unported. Successful
Huffman payload encoding and supplied-table sequence encoding do not change
either status.

The separate `huf_round_trip.c` target now has a complete adapted driver while
its source mapping remains partial. Exact tail selectors run in both BMI
consumption layouts, including requested depths, original-length capacities,
256 KiB source limits, early exits and guarded caller-output failure paths.
Independent Rust optimal trees, selected-description copying and one Rust
decoder remain distinct from C's tree policy, capacity-dependent serialization
and CPU/X1/X2 engines. All 833 pinned files ran both layouts under safe and
all-feature release builds: 1,666 identical per-file records per build, 3,332
total executions and no skipped files. The
[Huffman replay evidence](../../docs/evidence/2026-10-09-huffman-corpus.json)
retains each outcome and qualification.

Thirty official descriptions also yield byte-identical Rust 1X and 4X streams;
live C checks validate new sparse descriptions, standalone depths through
twelve, prepared dictionary trees, native-word capacity thresholds and both
C decoding engines. `tests/entropy_encoding.rs` proves zero-sequence
compression, inline/treeless lifetime, raw/RLE preservation, dictionary
replacement and fragmented output through public APIs. The inverse FSE and
sequence foundations separately exercise all 27 supplied predefined/RLE/new
channel combinations followed by repeat, long literal lengths, zero-literal
repeat rules and duplicate offsets. Their independent C oracle decodes all
27 frames and 54 sections.

The original dictionary campaigns now run separately from those synthetic
foundation tests. `tests/upstream_entropy_lifetimes.rs` preserves both prior
RNG paths at `zstreamtest.c:1831`, including the exact RDG bytes and serialized
dictionary ranges. It also executes all 43,691 six-byte flushes and nine
128-byte dictionary ranges at `:1884`. Independent C comparisons verify the
complete source/output bytes and frame boundaries; Rust's RLE small blocks
remain distinct from the original C raw blocks.

`tests/upstream/sequence_generator.rs` and
`tests/upstream/sequence_sweep.rs` preserve the
complete `zstreamtest.c:1551` schedule: 971 requests, 34,639,743 source bytes,
1,024-byte buffers and full-width XXH64 `d6a9522c4b14b2a5`. Every named Rust
strategy restores all source bytes. The actual C frame window is log 21,
inherited from explicit level-5 parameters before the later level-3 request
and session-only reset; the CDict, decoder limit and offset schedule reach log
24. Rust matches the advertised log-21 window. The C baseline retains two
workers and 4 MiB jobs; Rust remains single-threaded. Numeric levels, borrowed
CDict ownership and nondefault training profiles remain qualified, and the
bounded Rust search does not establish that every suggested offset code was
actually emitted.

Greedy, Chain and Lazy also passed all 512 pinned Silesia/GitHub directory
inputs, 12,407,963 bytes each, under safe and all-feature release builds with
no skipped files. Complete per-strategy frame fingerprints and retained
capacities agree across backends; see the
[strategy replay evidence](../../docs/evidence/2026-10-09-strategy-corpora.json).
This ports content checks with reused Rust contexts, not the upstream method,
level, dictionary, CLI or historical compressed-size CSV matrix.

These tests do not implement the fuzzer's level-19 block splitter,
`minMatch=7`, `splitAfterSequences`, btultra2, superblock, target compressed
block size or numeric level controls. In particular, `fuzzer.c:3265` retains
its original level-19 optimal-parser qualification. See
[the entropy coverage plan](../../docs/upstream-entropy-plan.md) for exact
completed checkpoints and remaining upstream obligations.

## Module 10 parameter and lifecycle coverage

`tests/upstream_parameters.rs` ports the original scalar adjustment, dictionary
profile equalities, bounds/default queries and qualified request-value subsets.
`tests/upstream_parameter_lifecycle.rs` separately adapts supported configuration,
getter preservation, dictionary/prefix lifetime and reset assertions. Ordering
and reset cases retain the original 10 KiB dictionary / 100 KiB source dimensions
with a declared deterministic RDG corpus, not inherited C buffer identity.
Full encoded-byte comparisons, parsed metadata and exact decoding supplement the
original reset size inequality. The explicit CLI oracle decodes thirty frames
per backend, including twelve restored-Greedy-default frames.

Six pure scalar diagnostic groups are now ported. Live subsets remain partial:
reference profiles do not configure compression, and numeric levels, LDM order
equivalence, seven-field live bulk setters and decoder parameter reset are not
implemented. Rust's checksum-enabled defaults, pledge clearing and explicit
reset after Finished are stated separately from C behavior. The matching
zstreamtest mappings retain their worker, opaque API and randomized-driver gaps.
See [parameter validation](../../docs/compression-parameters-validation.md) for
the focused, platform and independent-oracle evidence.

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

Bare Rust function names below refer to `tests/upstream_fuzzer.rs`; qualified
`file#function` references identify later streaming and dictionary ports.

| Upstream line | Original diagnostic label | Status | Rust test / missing capability |
| ---: | --- | --- | --- |
| 497 | compressBound | blocked | Public compression/decompression bound or frame scanning/skippable metadata API absent. |
| 519 | decompressBound | blocked | Public compression/decompression bound or frame scanning/skippable metadata API absent. |
| 576 | ZSTD_CCtx_setCParams | partial | `tests/upstream_parameters.rs#concrete_validation_retains_all_fields_and_rejects_original_invalid_values`. The original level-one valid profile and invalid windowLog 99 are checked through concrete value validation. This does not install seven compression fields in a live context: ZSTD_CCtx_setCParams, its allocation/setup path, and bulk context mutation remain unported. |
| 592 | Check block splitter doesn't oversplit incompressible data (seed %u) | blocked | Block-splitter, superblock, block-size, or btultra2 encoder strategy controls absent. |
| 670 | ZSTD_getErrorName | not-applicable | C numeric error/version/symbol-ABI contract; Rust uses typed Error and Cargo package version. |
| 675 | ZSTD_getErrorName with wrong value | not-applicable | C numeric error/version/symbol-ABI contract; Rust uses typed Error and Cargo package version. |
| 680 | min compression level | ported | `tests/upstream_parameters.rs#reference_minimum_and_default_levels_preserve_original_scalar_assertions`. The pinned minimum reference level is exposed and checked as -131072, including normalization below the minimum. This original group only queries the minimum; no operational numeric compression level is claimed. |
| 685 | default compression level | ported | `tests/upstream_parameters.rs#reference_minimum_and_default_levels_preserve_original_scalar_assertions`. The reference default-level getter assertion is preserved through DEFAULT_LEVEL == 3, default request construction, and level-zero normalization/query equivalence. The reference level is distinct from live named encoder policies. |
| 691 | ZSTD_versionNumber | not-applicable | C numeric error/version/symbol-ABI contract; Rust uses typed Error and Cargo package version. |
| 702 | ZSTD_adjustCParams | ported | `tests/upstream_parameters.rs#original_adjustment_example_reduces_hash_and_chain_for_source_and_dictionary`. The exact zero-initialized raw record with windowLog 10, hashLog 19 and chainLog 19 is adjusted for source 1000 and dictionary 100000; hashLog 18 and chainLog 17 are asserted. Rust uses typed known source size and validates the resulting concrete record. |
| 715 | compress %u bytes | partial | `tests/upstream_fuzzer.rs#basic_compression_and_decompression`. The original 5-MiB RDG input and full output comparison run through the existing named one-shot adapter. A separate no-dictionary Fast API now supports level 1, but this mapped adapter has not been switched to the original numeric call or reusable C context setup/sizeof reporting. Numeric API availability alone does not port this group. |
| 723 | size of cctx for level 1 | blocked | Reusable owned contexts and retained-capacity reporting exist; this original C static-workspace, custom-allocation, automatic-shrink or context-size-estimate contract remains unported. |
| 730 | decompress skippable frame -8 size | ported | `skippable_frame_minus_eight_size`. Exact malformed eight-byte input; rejects its overflowing declared payload. |
| 738 | ZSTD_getFrameContentSize test | ported | `frame_content_size`. Declared content-size value via FrameHeader::parse. |
| 744 | ZSTD_getDecompressedSize test | ported | `frame_content_size`. Deprecated size-query behavior expressed through typed header metadata. |
| 750 | ZSTD_findDecompressedSize test | partial | `frame_content_size`. Single-frame size is covered; a general concatenated-size scanning API is absent. |
| 756 | tight ZSTD_decompressBound test | blocked | Public compression/decompression bound or frame scanning/skippable metadata API absent. |
| 763 | ZSTD_decompressBound test with invalid srcSize | blocked | Public compression/decompression bound or frame scanning/skippable metadata API absent. |
| 770 | decompress %u bytes | ported | `basic_compression_and_decompression`. Original 5-MiB one-shot output and length. |
| 775 | decompress %u bytes with Huffman assembly disabled | partial | `basic_compression_and_decompression`. Byte-correct decoding covered; Rust has no assembly-selection parameter. |
| 786 | check decompressed result | ported | `basic_compression_and_decompression`. Every decompressed byte compared. |
| 793 | invalid endDirective | blocked | Typed Continue/Flush/Finish directives exist. The original C test injects invalid integer enum values 3 and -1, which the safe Rust enum does not represent; that numeric-ABI assertion is not ported or reclassified here. |
| 804 | ZSTD_checkCParams | ported | `tests/upstream_parameters.rs#concrete_validation_retains_all_fields_and_rejects_original_invalid_values`. The original default level-three, unknown-source, no-dictionary profile passes concrete seven-field validation. C public size zero is spelled SourceSize::Unknown in the Rust query. |
| 811 | ZSTD_createDCtx_advanced and ZSTD_sizeof_DCtx | blocked | Reusable owned contexts and retained-capacity reporting exist; this original C static-workspace, custom-allocation, automatic-shrink or context-size-estimate contract remains unported. |
| 820 | misc unaccounted for zstd symbols | not-applicable | C numeric error/version/symbol-ABI contract; Rust uses typed Error and Cargo package version. |
| 861 | decompress with null dict | partial | `tests/upstream_fuzzer.rs#basic_compression_and_decompression`. No-dictionary one-shot content behavior is covered; the original reusable context call with explicit null dictionary selection is not yet mapped. |
| 873 | decompress with null DDict | partial | `tests/upstream_fuzzer.rs#basic_compression_and_decompression`. No-dictionary one-shot content behavior is covered; the original reusable context call with explicit null prepared selection is not yet mapped. |
| 885 | decompress with 1 missing byte | ported | `decompress_with_one_missing_byte`. Truncated source rejected with UnexpectedEof. |
| 891 | decompress with 1 too much byte | ported | `decompress_with_one_extra_byte`. One trailing byte rejected with UnexpectedEof. |
| 897 | decompress too large input | ported | `decompress_with_large_trailing_input`. Overlong source rejected; deterministic zero tail replaces uninitialized C capacity. |
| 903 | decompress into NULL buffer | ported | `decompress_into_zero_output_limit`. Explicit zero output limit expresses the C zero-capacity contract. |
| 909 | decompress with corrupted checksum | partial | `decompress_with_corrupted_checksum`. All four checksum-byte corruptions rejected; forceIgnoreChecksum API absent. |
| 948 | ZSTD_decompressBound test with content size missing | blocked | Public compression/decompression bound or frame scanning/skippable metadata API absent. |
| 968 | check DCtx size is reduced after many oversized calls | blocked | Reusable owned contexts and retained-capacity reporting exist; this original C static-workspace, custom-allocation, automatic-shrink or context-size-estimate contract remains unported. |
| 1015 | ZSTD_compressCCtx() doesn't use advanced parameters | blocked | The original group first installs magicless output in a reusable CCtx, then checks that ZSTD_compressCCtx at level 1 ignores that advanced setting and emits standard magic. Whole-buffer Fast level 1 exists, but no mutable magicless CCtx setting or legacy-call parameter-clearing path is exposed, so this defining configuration-inheritance assertion remains unported. |
| 1020 | ZSTD_compress_usingDict() doesn't use advanced parameters | blocked | The original dictionary compression/context-copy, advanced parameter, static workspace or worker branch remains unported; prepared parsing and ordinary dictionary/prefix encoding and decoding are available. |
| 1025 | ZSTD_compress_usingCDict() doesn't use advanced parameters | blocked | This original dictionary compression, parameter/attachment strategy, context-copy/static workspace or multi-dictionary-cache branch is not ported; shared prepared parsing and ordinary dictionary/prefix encoding and decoding are available. |
| 1030 | ZSTD_compress_advanced() doesn't use advanced parameters | blocked | The original ZSTD_compress_advanced call uses the level-one parameter record after magicless format was installed in a reusable CCtx, and must emit standard magic by ignoring the prior advanced setting. Scalar parameter inspection and no-dictionary Fast request compression exist; mutable magicless context state and this legacy advanced-call inheritance contract remain unported. |
| 1035 | ZSTD_compress_usingCDict_advanced() doesn't use advanced parameters | blocked | This original dictionary compression, parameter/attachment strategy, context-copy/static workspace or multi-dictionary-cache branch is not ported; shared prepared parsing and ordinary dictionary/prefix encoding and decoding are available. |
| 1044 | maxBlockSize = 2K | blocked | Block-splitter, superblock, block-size, or btultra2 encoder strategy controls absent. |
| 1063 | ldm fill dict out-of-bounds check | blocked | The original dictionary compression/context-copy, advanced parameter, static workspace or worker branch remains unported; prepared parsing and ordinary dictionary/prefix encoding and decoding are available. |
| 1086 | testing dict compression with enableLdm and forceMaxWindow | blocked | The original dictionary compression/context-copy, advanced parameter, static workspace or worker branch remains unported; prepared parsing and ordinary dictionary/prefix encoding and decoding are available. |
| 1113 | testing dict compression for determinism | blocked | The original dictionary compression/context-copy, advanced parameter, static workspace or worker branch remains unported; prepared parsing and ordinary dictionary/prefix encoding and decoding are available. |
| 1157 | LDM + opt parser with small uncompressible block | blocked | Long-distance matching and optimal parser configuration/size comparison absent. |
| 1199 | testing ldm dictionary gets invalidated | blocked | This original dictionary compression, parameter/attachment strategy, context-copy/static workspace or multi-dictionary-cache branch is not ported; shared prepared parsing and ordinary dictionary/prefix encoding and decoding are available. |
| 1277 | testing refPrefx vs refPrefx + ldm (size comparison) | blocked | The original dictionary compression/context-copy, advanced parameter, static workspace or worker branch remains unported; prepared parsing and ordinary dictionary/prefix encoding and decoding are available. |
| 1344 | in-place decompression | blocked | Caller-owned overlapping input/output and decompression-margin API absent. |
| 1369 | in-place decompression with 2 frames | blocked | Caller-owned overlapping input/output and decompression-margin API absent. |
| 1398 | Check block splitter with 64K literal length | blocked | Block-splitter, superblock, block-size, or btultra2 encoder strategy controls absent. |
| 1467 | superblock uncompressible data: too many nocompress superblocks | blocked | Block-splitter, superblock, block-size, or btultra2 encoder strategy controls absent. |
| 1516 | superblock with no literals | blocked | Block-splitter, superblock, block-size, or btultra2 encoder strategy controls absent. |
| 1542 | superblock enough room for checksum | blocked | Block-splitter, superblock, block-size, or btultra2 encoder strategy controls absent. |
| 1554 | compress a NULL input with each level | partial | `tests/upstream_fuzzer.rs#compress_and_decompress_empty_frames`; `tests/upstream_reference_fast.rs#fuzzer_empty_level_loop_preserves_three_admitted_and_21_unsupported_profiles`. The exact original -1..22 range is enumerated. Empty-slice level and detached-request APIs produce equivalent valid empty frames for the three admitted Fast levels (-1, 1, 2); the remaining 21 levels are explicitly rejected as UnsupportedFeature. This does not execute all numeric strategies, NULL-pointer contracts, reusable C contexts or caller-owned destinations. Existing empty-frame coverage is retained. |
| 1567 | check CCtx size after compressing empty input | blocked | Reusable owned contexts and retained-capacity reporting exist; this original C static-workspace, custom-allocation, automatic-shrink or context-size-estimate contract remains unported. |
| 1577 | decompress empty frame into NULL | partial | `tests/upstream_fuzzer.rs#compress_and_decompress_empty_frames`, `tests/upstream_streaming.rs#flush_of_empty_unknown_size_frame_does_not_close_the_frame`. Known/unknown-size empty one-shot frames are covered. Streaming APIs exist, but this named group's exact original streaming subcase is not mapped here. |
| 1620 | reuse CCtx with expanding block size | blocked | Reusable encoder contexts, reset and source pledges exist. The original level-1 bufferless compressBegin_advanced/compressEnd reuse from a one-byte pledged source to 256 KiB of unknown-size CNBuffer remains unported. |
| 1637 | re-using a CCtx should compress the same | partial | `tests/upstream_streaming.rs#reset_repeats_identical_directive_and_chunk_schedules_deterministically`; `tests/upstream_reference_fast.rs#fuzzer_30_byte_initial_match_preserves_level_range_and_supported_output_equalities`. The exact original 30-byte source (0..19 followed by 0..9) and 1..19 range are preserved. Levels 1 and 2 produce identical complete bytes across repeated level calls and detached-request calls, followed by exact decoding; 17 other profiles are explicitly unsupported. Earlier deterministic named streaming reset coverage remains. Reusable numeric CCtx, the caller-destination compressBound-1 branch, forced streaming, and complete one-shot/context/stream equivalence remain unported. |
| 1681 | btultra2 & 1st block | blocked | Block-splitter, superblock, block-size, or btultra2 encoder strategy controls absent. |
| 1721 | ZSTD_CCtx_getParameter() | partial | `tests/upstream_parameters.rs#requested_hash_override_survives_level_changes_and_default_replacement`; `tests/upstream_parameters.rs#explicit_override_order_and_zero_auto_preserve_requested_and_resolved_values`; `tests/upstream_parameters.rs#request_resolution_preserves_the_two_pass_empty_source_chain_witness`; `tests/upstream_parameter_lifecycle.rs#settings_survive_partial_output_flush_finish_and_rejected_configuration`; `tests/upstream_parameter_lifecycle.rs#session_reset_preserves_dictionary_and_parameter_reset_clears_it`; `tests/upstream_search_lifecycle.rs#original_getter_start_session_reset_and_parameter_reset_sequence`. Detached reference request getters preserve default level/hash auto, explicit hashLog 6 across level-seven changes, and restoration by Default. A separate live search test follows the original getter/start/session-reset/parameter-reset sequence for all three named policies without allocation or owner cloning. Live Rust hash controls have concrete default 16/minimum 10, unlike C automatic 0/minimum 6; numeric level getters are not attached to the live named Encoder; separate whole-buffer APIs execute admitted Fast requests. Other supported live settings survive partial headers, input, Flush, Finish and rejected configuration. Rust retains its explicit Finished/reset protocol and checksum/pledge defaults. |
| 1764 | ZSTD_CCtx_setCParams() | partial | `tests/upstream_parameters.rs#concrete_validation_retains_all_fields_and_rejects_original_invalid_values`; `tests/upstream_parameter_lifecycle.rs#frame_flags_and_invalid_windows_preserve_atomic_supported_configuration`. Concrete records reject the original invalid strategy u32::MAX and preserve all seven valid level-derived fields, including level 12. Supported live option replacement rejects invalid windows atomically. There is no seven-field ZSTD_CCtx_setCParams bulk installer, so the original live all-fields getter and invalid-strategy bulk atomicity assertions remain unported. |
| 1808 | ZSTD_CCtx_setFParams() | partial | `tests/upstream_parameters.rs#reference_frame_defaults_remain_separate_from_live_encoder_defaults`; `tests/upstream_parameter_lifecycle.rs#frame_flags_and_invalid_windows_preserve_atomic_supported_configuration`. Reference frame defaults preserve content-size=true, checksum=false and dictionary-ID=true. Supported live option replacement verifies false/true/false for the original {0,1,1} FParams flags, as well as emitted metadata and exact decoding. Rust live defaults enable checksum, and no separate C-style ZSTD_CCtx_setFParams/getParameter API is claimed. |
| 1832 | ZSTD_CCtx_setParams() | partial | `tests/upstream_parameters.rs#concrete_validation_retains_all_fields_and_rejects_original_invalid_values`; `tests/upstream_parameters.rs#reference_frame_defaults_remain_separate_from_live_encoder_defaults`; `tests/upstream_parameter_lifecycle.rs#frame_flags_and_invalid_windows_preserve_atomic_supported_configuration`; `tests/upstream_parameter_lifecycle.rs#settings_survive_partial_output_flush_finish_and_rejected_configuration`. The scalar subset validates all seven level-derived fields and rejects invalid strategy; the live subset verifies supported frame flags, getter preservation and invalid-window atomicity. Combined compression/frame bulk installation and invalid-strategy rollback across all original live numeric fields remain unported. Reference checksum-disabled defaults remain separate from Rust checksum-enabled defaults. |
| 1891 | ldm conditionally enabled by default doesn't change cctx params | blocked | Long-distance matching and optimal parser configuration/size comparison absent. |
| 1912 | overflow protection with large windowLog | blocked | The original level -999 campaign repeatedly reuses a CCtx with unknown source size, disabled content size and ZSTD_WINDOWLOG_MAX for enough 5-MiB compressions to cross the U32 index boundary. Whole-buffer negative Fast profiles exist, but the 1-MiB operational window cap, known-size one-shot lifetime and checked usize indices do not implement this large-window C index-rescaling/context-reuse stress contract. |
| 1929 | size down context | blocked | Reusable owned contexts and retained-capacity reporting exist; this original C static-workspace, custom-allocation, automatic-shrink or context-size-estimate contract remains unported. |
| 1964 | create static CCtx for level %u | blocked | Reusable owned contexts and retained-capacity reporting exist; this original C static-workspace, custom-allocation, automatic-shrink or context-size-estimate contract remains unported. |
| 1988 | compress small input with small static CCtx | blocked | Reusable owned contexts and retained-capacity reporting exist; this original C static-workspace, custom-allocation, automatic-shrink or context-size-estimate contract remains unported. |
| 1995 | compress large input with small static CCtx (must fail) | blocked | Reusable owned contexts and retained-capacity reporting exist; this original C static-workspace, custom-allocation, automatic-shrink or context-size-estimate contract remains unported. |
| 2003 | resize context to full CCtx size | blocked | Reusable owned contexts and retained-capacity reporting exist; this original C static-workspace, custom-allocation, automatic-shrink or context-size-estimate contract remains unported. |
| 2009 | compress large input with static CCtx | blocked | Reusable owned contexts and retained-capacity reporting exist; this original C static-workspace, custom-allocation, automatic-shrink or context-size-estimate contract remains unported. |
| 2016 | compress small input often enough to trigger context reduce | blocked | Reusable owned contexts and retained-capacity reporting exist; this original C static-workspace, custom-allocation, automatic-shrink or context-size-estimate contract remains unported. |
| 2028 | init CCtx for level %u | blocked | Reusable owned contexts and retained-capacity reporting exist; this original C static-workspace, custom-allocation, automatic-shrink or context-size-estimate contract remains unported. |
| 2032 | compression again with static CCtx | blocked | Reusable owned contexts and retained-capacity reporting exist; this original C static-workspace, custom-allocation, automatic-shrink or context-size-estimate contract remains unported. |
| 2039 | simple decompression test with static DCtx | blocked | Reusable owned contexts and retained-capacity reporting exist; this original C static-workspace, custom-allocation, automatic-shrink or context-size-estimate contract remains unported. |
| 2046 | check decompressed result | blocked | Reusable owned contexts and retained-capacity reporting exist; this original C static-workspace, custom-allocation, automatic-shrink or context-size-estimate contract remains unported. |
| 2050 | init CCtx for too large level (must fail) | blocked | Reusable owned contexts and retained-capacity reporting exist; this original C static-workspace, custom-allocation, automatic-shrink or context-size-estimate contract remains unported. |
| 2055 | init CCtx for small level %u (should work again) | blocked | Reusable owned contexts and retained-capacity reporting exist; this original C static-workspace, custom-allocation, automatic-shrink or context-size-estimate contract remains unported. |
| 2059 | use CStream on CCtx-sized static context (should fail) | blocked | Reusable owned contexts and retained-capacity reporting exist; this original C static-workspace, custom-allocation, automatic-shrink or context-size-estimate contract remains unported. |
| 2068 | resize context to CStream size, then stream compress | blocked | Reusable owned contexts and retained-capacity reporting exist; this original C static-workspace, custom-allocation, automatic-shrink or context-size-estimate contract remains unported. |
| 2078 | CStream for small level %u | blocked | Reusable owned contexts and retained-capacity reporting exist; this original C static-workspace, custom-allocation, automatic-shrink or context-size-estimate contract remains unported. |
| 2086 | init static CStream with dictionary (should fail) | blocked | This original dictionary compression, parameter/attachment strategy, context-copy/static workspace or multi-dictionary-cache branch is not ported; shared prepared parsing and ordinary dictionary/prefix encoding and decoding are available. |
| 2091 | use DStream on DCtx-sized static context (should fail) | blocked | Reusable owned contexts and retained-capacity reporting exist; this original C static-workspace, custom-allocation, automatic-shrink or context-size-estimate contract remains unported. |
| 2100 | test estimation functions with default cctx params | blocked | Reusable owned contexts and retained-capacity reporting exist; this original C static-workspace, custom-allocation, automatic-shrink or context-size-estimate contract remains unported. |
| 2142 | test estimation functions with maxBlockSize = 0 | blocked | Reusable owned contexts and retained-capacity reporting exist; this original C static-workspace, custom-allocation, automatic-shrink or context-size-estimate contract remains unported. |
| 2192 | Static context sizes for negative levels | blocked | Reusable owned contexts and retained-capacity reporting exist; this original C static-workspace, custom-allocation, automatic-shrink or context-size-estimate contract remains unported. |
| 2205 | create ZSTDMT CCtx | blocked | Multithreaded compressor and shared thread-pool API absent. |
| 2216 | compress %u bytes with 2 threads | blocked | Multithreaded compressor and shared thread-pool API absent. |
| 2222 | decompressed size test | blocked | Multithreaded compressor and shared thread-pool API absent. |
| 2230 | decompress %u bytes | blocked | Multithreaded compressor and shared thread-pool API absent. |
| 2235 | check decompressed result | blocked | Multithreaded compressor and shared thread-pool API absent. |
| 2242 | compress -T2 with checksum | blocked | Multithreaded compressor and shared thread-pool API absent. |
| 2251 | decompress %u bytes | blocked | Multithreaded compressor and shared thread-pool API absent. |
| 2259 | compress empty string and decompress with small window log | blocked | Encoder content-size omission and Decoder window limits exist. This exact empty-frame/window-log 10/zero-output original setup has not yet been mapped. |
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
| 2443 | copy context too soon | blocked | The original dictionary compression/context-copy, advanced parameter, static workspace or worker branch remains unported; prepared parsing and ordinary dictionary/prefix encoding and decoding are available. |
| 2448 | load dictionary into context | blocked | This original dictionary compression, parameter/attachment strategy, context-copy/static workspace or multi-dictionary-cache branch is not ported; shared prepared parsing and ordinary dictionary/prefix encoding and decoding are available. |
| 2453 | compress with flat dictionary | partial | `tests/upstream_dictionary_encode.rs#raw_and_formatted_dictionary_matches_reduce_size_for_independent_source_bytes`. `tests/upstream_dictionary_encode.rs#official_codec_decodes_raw_prepared_and_omitted_id_dictionary_frames`. `tests/upstream_dictionary_encode.rs#official_codec_decodes_one_shot_dictionary_frames_across_content_size_boundaries`. Raw/formatted shared dictionary compression now creates useful matches with exact Rust and independent C decoding. Original level selection, generated CNBuffer workload and specialized C preparation variants remain unported. |
| 2461 | frame built with flat dictionary should be decompressible | partial | `tests/upstream_dictionary.rs#official_raw_prepared_trained_and_duplicate_repeat_frames_decode_exactly`. `tests/upstream_dictionary.rs#one_shot_dictionary_helper_matches_streaming_and_preserves_shared_handles`. Raw and shared prepared dictionary decoding restores exact official-oracle fixtures under one-shot and fragmented input/output. The original 551-byte RDG dictionary and CNBuffer workload have not been replayed. |
| 2469 | compress with duplicated context | blocked | The original dictionary compression/context-copy, advanced parameter, static workspace or worker branch remains unported; prepared parsing and ordinary dictionary/prefix encoding and decoding are available. |
| 2480 | frame built with duplicated context should be decompressible | blocked | The original dictionary compression/context-copy, advanced parameter, static workspace or worker branch remains unported; prepared parsing and ordinary dictionary/prefix encoding and decoding are available. |
| 2488 | decompress with DDict | partial | `tests/upstream_dictionary.rs#official_raw_prepared_trained_and_duplicate_repeat_frames_decode_exactly`. `tests/upstream_dictionary.rs#one_shot_dictionary_helper_matches_streaming_and_preserves_shared_handles`. Raw and shared prepared dictionary decoding restores exact official-oracle fixtures under one-shot and fragmented input/output. The original 551-byte RDG dictionary and CNBuffer workload have not been replayed. |
| 2496 | decompress with static DDict | blocked | This original dictionary compression, parameter/attachment strategy, context-copy/static workspace or multi-dictionary-cache branch is not ported; shared prepared parsing and ordinary dictionary/prefix encoding and decoding are available. |
| 2508 | check content size on duplicated context | blocked | The original dictionary compression/context-copy, advanced parameter, static workspace or worker branch remains unported; prepared parsing and ordinary dictionary/prefix encoding and decoding are available. |
| 2554 | flat-dictionary efficiency test | partial | `tests/upstream_dictionary_encode.rs#one_shot_and_streaming_dictionary_compression_both_use_real_dictionary_matches`. Independent source bytes compress to less than one quarter the no-dictionary size in one-shot and streaming APIs and decode exactly. The original strategy/level efficiency loop is not reproduced. |
| 2665 | dictBuilder on cyclic data | ported | `tests/upstream_formatted_training.rs#upstream_cyclic_training_shape_builds_a_formatted_dictionary_with_a_nonzero_id`. The exact 1 MiB index-modulo-256 source, 128 samples of 8 KiB, 16 KiB dictionary capacity and training-success assertion run through the independent Rust formatted trainer. The result has bounded content, nonzero ID, generated entropy metadata and an exact roundtrip. This case asserts successful construction, not byte identity with the upstream training algorithm. |
| 2675 | dictBuilder | ported | `tests/upstream_training.rs#trained_content_improves_held_out_messages_and_roundtrips_exactly`, `tests/upstream_training.rs#official_codec_uses_rust_trained_content_for_independent_held_out_roundtrips`, `tests/upstream_training.rs#training_is_independent_of_allocation_addresses_and_releases_sample_borrows`, `tests/upstream_formatted_training.rs#trained_formatted_content_improves_held_out_messages_and_roundtrips`, `tests/upstream_formatted_training.rs#official_codec_loads_learned_metadata_and_roundtrips_in_both_directions`, `tests/upstream_formatted_training.rs#large_caps_do_not_force_large_allocations_and_dictionary_bytes_own_their_sources`, `tests/upstream_formatted_training.rs#official_cnbuffer_campaign_trains_and_roundtrips_the_complete_source`. The original generic training-success assertion runs on exact official RDG-generated 5-MiB CNBuffer sources: first 1 MiB as 128 samples of 8 KiB and 16 KiB dictionary capacity. Five seed/profile campaigns train successfully, match independently C-read IDs/header sizes, and recover every source byte with Rust and C. Independent Rust training need not reproduce upstream dictionary bytes; strategy-specific COVER/fastCover groups remain blocked. |
| 2682 | Multithreaded COVER dictBuilder | blocked | The independent Rust raw/formatted trainer exists, but COVER four-worker training with the original steps/sample schedule is not implemented. Successful formatted construction does not substitute for these strategy controls. |
| 2696 | COVER dictBuilder with shrinkDict | blocked | The independent Rust raw/formatted trainer exists, but COVER shrinkDict and maximum-regression optimization is not implemented. Successful formatted construction does not substitute for these strategy controls. |
| 2712 | Multithreaded FASTCOVER dictBuilder | blocked | The independent Rust raw/formatted trainer exists, but fastCover four-worker training with the original steps/sample schedule is not implemented. Successful formatted construction does not substitute for these strategy controls. |
| 2726 | FASTCOVER dictBuilder with shrinkDict | blocked | The independent Rust raw/formatted trainer exists, but fastCover shrinkDict and maximum-regression optimization is not implemented. Successful formatted construction does not substitute for these strategy controls. |
| 2742 | check dictID | ported | `src/dictionary.rs#official_formatted_dictionary_metadata_matches_independent_c_loader`, `tests/upstream_formatted_training.rs#upstream_cyclic_training_shape_builds_a_formatted_dictionary_with_a_nonzero_id`, `tests/upstream_formatted_training.rs#identifiers_are_explicit_or_derived_from_original_content_before_padding`, `tests/upstream_formatted_training.rs#official_codec_loads_learned_metadata_and_roundtrips_in_both_directions`. Rust-generated formatted dictionaries have nonzero IDs; explicit IDs and automatic public-range IDs are verified. This named metadata assertion is complete while its preceding COVER/fastCover/shrink/thread training groups remain blocked. |
| 2747 | check dict header size no error | ported | `src/dictionary.rs#official_formatted_dictionary_metadata_matches_independent_c_loader`, `tests/upstream_formatted_training.rs#upstream_cyclic_training_shape_builds_a_formatted_dictionary_with_a_nonzero_id`, `tests/upstream_formatted_training.rs#finalization_preserves_short_content_padding_and_long_content_suffixes`, `tests/upstream_formatted_training.rs#official_codec_loads_learned_metadata_and_roundtrips_in_both_directions`. The generated formatted dictionary has a nonempty validated entropy header and bounded content; independent C loading accepts its metadata. This named header assertion does not complete the preceding strategy-specific training groups. |
| 2752 | check dict header size correctness | ported | `src/dictionary.rs#official_formatted_dictionary_metadata_matches_independent_c_loader`. Prepared metadata tests verify nonzero dictionary IDs and exact header/content boundaries, including the original fixed-training dictionary ID 99 and header length115. |
| 2758 | compress with dictionary | partial | `tests/upstream_dictionary_encode.rs#raw_and_formatted_dictionary_matches_reduce_size_for_independent_source_bytes`, `tests/upstream_dictionary_encode.rs#official_codec_decodes_raw_prepared_and_omitted_id_dictionary_frames`, `tests/upstream_dictionary_encode.rs#official_codec_decodes_one_shot_dictionary_frames_across_content_size_boundaries`, `tests/upstream_formatted_training.rs#trained_formatted_content_improves_held_out_messages_and_roundtrips`, `tests/upstream_formatted_training.rs#official_codec_loads_learned_metadata_and_roundtrips_in_both_directions`, `tests/upstream_formatted_training.rs#official_cnbuffer_campaign_trains_and_roundtrips_the_complete_source`. Useful dictionary compression covers exact official CNBuffer campaigns and C/Rust-generated formatted dictionaries, with full Rust and independent C decoding. Original level 4 and specialized C preparation/strategy parameter setup remain unported. |
| 2765 | retrieve dictID from dictionary | ported | `src/dictionary.rs#official_formatted_dictionary_metadata_matches_independent_c_loader`, `tests/upstream_formatted_training.rs#identifiers_are_explicit_or_derived_from_original_content_before_padding`, `tests/upstream_formatted_training.rs#upstream_cyclic_training_shape_builds_a_formatted_dictionary_with_a_nonzero_id`. Dictionary preparation exposes the exact generated or explicit dictionary ID, including all wire widths and the original fixed dictionary ID 99. |
| 2771 | retrieve dictID from frame | ported | `tests/upstream_dictionary.rs#prepared_http_dictionary_decodes_the_official_golden_frame`, `tests/upstream_formatted_training.rs#identifiers_are_explicit_or_derived_from_original_content_before_padding`, `tests/upstream_formatted_training.rs#upstream_cyclic_training_shape_builds_a_formatted_dictionary_with_a_nonzero_id`, `tests/upstream_formatted_training.rs#official_codec_loads_learned_metadata_and_roundtrips_in_both_directions`. FrameHeader reports the same nonzero ID as the Rust-trained/finalized dictionary across automatic IDs and every explicit wire width. |
| 2777 | frame built with dictionary should be decompressible | partial | `tests/upstream_dictionary.rs#official_raw_prepared_trained_and_duplicate_repeat_frames_decode_exactly`, `tests/upstream_dictionary.rs#one_shot_dictionary_helper_matches_streaming_and_preserves_shared_handles`, `tests/upstream_formatted_training.rs#trained_formatted_content_improves_held_out_messages_and_roundtrips`, `tests/upstream_formatted_training.rs#official_codec_loads_learned_metadata_and_roundtrips_in_both_directions`, `tests/upstream_formatted_training.rs#official_cnbuffer_campaign_trains_and_roundtrips_the_complete_source`. C-generated and Rust-trained dictionaries decode exact one-shot/fragmented content; five full 5-MiB official CNBuffer campaigns match both Rust and independent C decoding. Original level 4/C-specific frame-construction parameters remain unported. |
| 2788 | estimate CDict size | blocked | This original dictionary compression, parameter/attachment strategy, context-copy/static workspace or multi-dictionary-cache branch is not ported; shared prepared parsing and ordinary dictionary/prefix encoding and decoding are available. |
| 2794 | compress with CDict | partial | `tests/upstream_dictionary_encode.rs#raw_and_formatted_dictionary_matches_reduce_size_for_independent_source_bytes`, `tests/upstream_dictionary_encode.rs#official_codec_decodes_raw_prepared_and_omitted_id_dictionary_frames`, `tests/upstream_dictionary_encode.rs#official_codec_decodes_one_shot_dictionary_frames_across_content_size_boundaries`, `tests/upstream_formatted_training.rs#trained_formatted_content_improves_held_out_messages_and_roundtrips`, `tests/upstream_formatted_training.rs#official_codec_loads_learned_metadata_and_roundtrips_in_both_directions`, `tests/upstream_formatted_training.rs#official_cnbuffer_campaign_trains_and_roundtrips_the_complete_source`. Useful dictionary compression covers exact official CNBuffer campaigns and C/Rust-generated formatted dictionaries, with full Rust and independent C decoding. Original level 4 and specialized C preparation/strategy parameter setup remain unported. |
| 2809 | retrieve dictID from frame | ported | `tests/upstream_dictionary.rs#prepared_http_dictionary_decodes_the_official_golden_frame`, `tests/upstream_formatted_training.rs#identifiers_are_explicit_or_derived_from_original_content_before_padding`, `tests/upstream_formatted_training.rs#upstream_cyclic_training_shape_builds_a_formatted_dictionary_with_a_nonzero_id`, `tests/upstream_formatted_training.rs#official_codec_loads_learned_metadata_and_roundtrips_in_both_directions`. Shared prepared dictionary compression writes the generated or explicit ID; parsed frame metadata equals the selected dictionary ID across all field widths. |
| 2815 | frame built with dictionary should be decompressible | partial | `tests/upstream_dictionary.rs#official_raw_prepared_trained_and_duplicate_repeat_frames_decode_exactly`, `tests/upstream_dictionary.rs#one_shot_dictionary_helper_matches_streaming_and_preserves_shared_handles`, `tests/upstream_formatted_training.rs#trained_formatted_content_improves_held_out_messages_and_roundtrips`, `tests/upstream_formatted_training.rs#official_codec_loads_learned_metadata_and_roundtrips_in_both_directions`, `tests/upstream_formatted_training.rs#official_cnbuffer_campaign_trains_and_roundtrips_the_complete_source`. C-generated and Rust-trained dictionaries decode exact one-shot/fragmented content; five full 5-MiB official CNBuffer campaigns match both Rust and independent C decoding. Original level 4/C-specific frame-construction parameters remain unported. |
| 2826 | compress with static CDict | blocked | This original dictionary compression, parameter/attachment strategy, context-copy/static workspace or multi-dictionary-cache branch is not ported; shared prepared parsing and ordinary dictionary/prefix encoding and decoding are available. |
| 2854 | ZSTD_compress_usingCDict_advanced, no contentSize, no dictID | partial | `tests/upstream_dictionary_encode.rs#omitted_dictionary_id_preserves_match_effectiveness_and_checksum`. `tests/upstream_dictionary_encode.rs#dictionary_frames_cover_known_unknown_size_checksum_and_window_options`. Dictionary encoding supports explicit ID omission, checksum and known/unknown content-size flags with exact roundtrips. Original CDict_advanced compression parameter setup/levels are not reproduced. |
| 2868 | try retrieving contentSize from frame | ported | `tests/upstream_dictionary_encode.rs#dictionary_frames_cover_known_unknown_size_checksum_and_window_options`. Frames encoded with include_content_size=false parse with unknown content size, including dictionary-bearing streaming frames. |
| 2874 | frame built without dictID should be decompressible | partial | `tests/upstream_dictionary_encode.rs#dictionary_frames_cover_known_unknown_size_checksum_and_window_options`. `tests/upstream_dictionary_encode.rs#omitted_dictionary_id_preserves_match_effectiveness_and_checksum`. `tests/upstream_dictionary.rs#official_omitted_id_and_prefix_first_frames_require_content_without_an_id`. Omitted-ID dictionary frames, including explicit known/unknown content-size variants, roundtrip exactly; omitted-ID input still fails without selected content. Original advanced encoder strategy/level setup remains unported. |
| 2886 | ZSTD_compress_advanced, no dictID | partial | `tests/upstream_dictionary_encode.rs#omitted_dictionary_id_preserves_match_effectiveness_and_checksum`. `tests/upstream_dictionary_encode.rs#dictionary_frames_cover_known_unknown_size_checksum_and_window_options`. Dictionary encoding supports explicit ID omission, checksum and known/unknown content-size flags with exact roundtrips. Original CDict_advanced compression parameter setup/levels are not reproduced. |
| 2896 | frame built without dictID should be decompressible | partial | `tests/upstream_dictionary_encode.rs#dictionary_frames_cover_known_unknown_size_checksum_and_window_options`. `tests/upstream_dictionary_encode.rs#omitted_dictionary_id_preserves_match_effectiveness_and_checksum`. `tests/upstream_dictionary.rs#official_omitted_id_and_prefix_first_frames_require_content_without_an_id`. Omitted-ID dictionary frames, including explicit known/unknown content-size variants, roundtrip exactly; omitted-ID input still fails without selected content. Original advanced encoder strategy/level setup remains unported. |
| 2907 | dictionary containing only header should return error | ported | `src/dictionary.rs#official_invalid_repeat_dictionary_and_header_only_fixture_are_rejected`. The exact eight-byte header-only dictionary is rejected with InvalidDictionary in Auto and Formatted preparation modes. |
| 2920 | bufferless api with cdict | blocked | This original dictionary compression, parameter/attachment strategy, context-copy/static workspace or multi-dictionary-cache branch is not ported; shared prepared parsing and ordinary dictionary/prefix encoding and decoding are available. |
| 2941 | Building cdict w/ ZSTD_dct_fullDict on a good dictionary | partial | `src/dictionary.rs#official_formatted_dictionary_metadata_matches_independent_c_loader`. `src/dictionary.rs#raw_mode_never_interprets_even_valid_or_corrupt_formatted_headers`. `tests/upstream_dictionary_fuzz.rs#dictionary_loader_covers_all_modes_prefixes_and_load_method_selectors`. `tests/upstream_dictionary_fuzz.rs#dictionary_fuzz_adapters_replay_deterministic_noise_and_truncated_dictionaries`. Auto/Raw/Formatted preparation, malformed/header-looking raw input and successful supported-encoder roundtrips are exercised by the exact tail-selected loader adapter. C-specific CDict parameter choices and the original shifted-buffer setup remain separate. |
| 2949 | Building cdict w/ ZSTD_dct_fullDict on a rawContent (must fail) | partial | `src/dictionary.rs#official_formatted_dictionary_metadata_matches_independent_c_loader`. `src/dictionary.rs#raw_mode_never_interprets_even_valid_or_corrupt_formatted_headers`. `tests/upstream_dictionary_fuzz.rs#dictionary_loader_covers_all_modes_prefixes_and_load_method_selectors`. `tests/upstream_dictionary_fuzz.rs#dictionary_fuzz_adapters_replay_deterministic_noise_and_truncated_dictionaries`. Auto/Raw/Formatted preparation, malformed/header-looking raw input and successful supported-encoder roundtrips are exercised by the exact tail-selected loader adapter. C-specific CDict parameter choices and the original shifted-buffer setup remain separate. |
| 2963 | Loading rawContent starting with dict header w/ ZSTD_dct_auto should fail | partial | `src/dictionary.rs#every_formatted_truncation_obeys_metadata_and_repeat_content_bounds`. `src/dictionary.rs#official_invalid_repeat_dictionary_and_header_only_fixture_are_rejected`. `tests/upstream_dictionary_fuzz.rs#dictionary_loader_covers_all_modes_prefixes_and_load_method_selectors`. `tests/upstream_dictionary_fuzz.rs#dictionary_fuzz_adapters_replay_deterministic_noise_and_truncated_dictionaries`. Auto/Raw/Formatted preparation, malformed/header-looking raw input and successful supported-encoder roundtrips are exercised by the exact tail-selected loader adapter. C-specific CDict parameter choices and the original shifted-buffer setup remain separate. |
| 2976 | Loading rawContent starting with dict header w/ ZSTD_dct_rawContent should pass | partial | `src/dictionary.rs#raw_mode_never_interprets_even_valid_or_corrupt_formatted_headers`. `tests/upstream_dictionary_fuzz.rs#dictionary_loader_covers_all_modes_prefixes_and_load_method_selectors`. `tests/upstream_dictionary_fuzz.rs#dictionary_fuzz_adapters_replay_deterministic_noise_and_truncated_dictionaries`. Auto/Raw/Formatted preparation, malformed/header-looking raw input and successful supported-encoder roundtrips are exercised by the exact tail-selected loader adapter. C-specific CDict parameter choices and the original shifted-buffer setup remain separate. |
| 2987 | Testing non-attached CDict with ZSTD_dct_rawContent | blocked | This original dictionary compression, parameter/attachment strategy, context-copy/static workspace or multi-dictionary-cache branch is not ported; shared prepared parsing and ordinary dictionary/prefix encoding and decoding are available. |
| 3001 | ZSTD_CCtx_refCDict() then set parameters | partial | `tests/upstream_parameter_lifecycle.rs#dictionary_and_supported_options_order_produce_identical_complete_frames`; `tests/upstream_parameter_lifecycle.rs#frame_flags_and_invalid_windows_preserve_atomic_supported_configuration`; `tests/upstream_parameter_lifecycle.rs#official_codec_decodes_configured_session_and_parameter_reset_frames`; `tests/upstream_search_lifecycle.rs#original_hash_twelve_before_and_after_raw_formatted_shared_and_borrowed_attachment`. Operational hashLog 12 is installed and read on both sides of dictionary attachment for raw/formatted dictionaries, shared/borrowed handles and all three named policies. Each flow verifies complete decoding and byte-identical session reuse. Earlier supported-options frames are separately C-decoded. The original profile-specific CDict, dictionary-aware numeric level-one execution and C attachment-parameter inheritance remain unimplemented; the new numeric Fast entry points are no-dictionary whole-buffer APIs. |
| 3013 | Loading dictionary before setting parameters is the same as loading after | partial | `tests/upstream_parameter_lifecycle.rs#dictionary_and_supported_options_order_produce_identical_complete_frames`; `tests/upstream_parameter_lifecycle.rs#official_codec_decodes_configured_session_and_parameter_reset_frames`. Both dictionary/options installation orders produce identical complete frames and exact decoding for Greedy, Chain and Lazy. Original 10 KiB dictionary/100 KiB source dimensions are retained using declared RDG(0.5,0.0,seed 0) data and its first 10 KiB, not the inherited mutable C CNBuffer. Operational level seven and C-specific dictionary preparation remain unported. |
| 3032 | Loading a dictionary clears the prefix | partial | `tests/upstream_dictionary_encode.rs#prefix_and_dictionary_configuration_replace_one_another`. `tests/upstream_dictionary.rs#setting_a_prefix_replaces_the_persistent_dictionary`. `tests/upstream_dictionary.rs#setting_or_clearing_a_dictionary_replaces_any_pending_prefix`. Setting a persistent dictionary or one-use prefix replaces the other configuration and selects the expected frame ID/content. Rust unifies raw/prepared dictionary ownership; original separate C dictionary/CDict APIs and byte-identical encoder comparisons remain unported. |
| 3040 | Loading a dictionary clears the cdict | partial | `tests/upstream_dictionary_encode.rs#prefix_and_dictionary_configuration_replace_one_another`. `tests/upstream_dictionary.rs#setting_a_prefix_replaces_the_persistent_dictionary`. `tests/upstream_dictionary.rs#setting_or_clearing_a_dictionary_replaces_any_pending_prefix`. Setting a persistent dictionary or one-use prefix replaces the other configuration and selects the expected frame ID/content. Rust unifies raw/prepared dictionary ownership; original separate C dictionary/CDict APIs and byte-identical encoder comparisons remain unported. |
| 3050 | Loading a cdict clears the prefix | partial | `tests/upstream_dictionary_encode.rs#prefix_and_dictionary_configuration_replace_one_another`. `tests/upstream_dictionary.rs#setting_a_prefix_replaces_the_persistent_dictionary`. `tests/upstream_dictionary.rs#setting_or_clearing_a_dictionary_replaces_any_pending_prefix`. Setting a persistent dictionary or one-use prefix replaces the other configuration and selects the expected frame ID/content. Rust unifies raw/prepared dictionary ownership; original separate C dictionary/CDict APIs and byte-identical encoder comparisons remain unported. |
| 3060 | Loading a cdict clears the dictionary | partial | `tests/upstream_dictionary_encode.rs#prefix_and_dictionary_configuration_replace_one_another`. `tests/upstream_dictionary.rs#setting_a_prefix_replaces_the_persistent_dictionary`. `tests/upstream_dictionary.rs#setting_or_clearing_a_dictionary_replaces_any_pending_prefix`. Setting a persistent dictionary or one-use prefix replaces the other configuration and selects the expected frame ID/content. Rust unifies raw/prepared dictionary ownership; original separate C dictionary/CDict APIs and byte-identical encoder comparisons remain unported. |
| 3070 | Loading a prefix clears the dictionary | partial | `tests/upstream_dictionary_encode.rs#prefix_and_dictionary_configuration_replace_one_another`. `tests/upstream_dictionary.rs#setting_a_prefix_replaces_the_persistent_dictionary`. `tests/upstream_dictionary.rs#setting_or_clearing_a_dictionary_replaces_any_pending_prefix`. Setting a persistent dictionary or one-use prefix replaces the other configuration and selects the expected frame ID/content. Rust unifies raw/prepared dictionary ownership; original separate C dictionary/CDict APIs and byte-identical encoder comparisons remain unported. |
| 3078 | Loading a prefix clears the cdict | partial | `tests/upstream_dictionary_encode.rs#prefix_and_dictionary_configuration_replace_one_another`. `tests/upstream_dictionary.rs#setting_a_prefix_replaces_the_persistent_dictionary`. `tests/upstream_dictionary.rs#setting_or_clearing_a_dictionary_replaces_any_pending_prefix`. Setting a persistent dictionary or one-use prefix replaces the other configuration and selects the expected frame ID/content. Rust unifies raw/prepared dictionary ownership; original separate C dictionary/CDict APIs and byte-identical encoder comparisons remain unported. |
| 3088 | Loaded dictionary persists across reset session | partial | `tests/upstream_dictionary_encode.rs#encoder_reset_and_reset_with_options_preserve_persistent_dictionary`; `tests/upstream_parameter_lifecycle.rs#session_reset_preserves_dictionary_and_parameter_reset_clears_it`; `tests/upstream_parameter_lifecycle.rs#official_codec_decodes_configured_session_and_parameter_reset_frames`. Persistent dictionary selection and complete deterministic frame bytes survive session reset with the original 10 KiB/100 KiB dimensions and declared RDG(0.5,0.0,seed 0) corpus under each named policy. Rust reset clears the pledge, so the same supported options are explicitly restored; this does not claim the inherited C corpus, numeric default-level execution or automatic next-frame protocol. |
| 3104 | Loaded dictionary is cleared after resetting parameters | partial | `tests/upstream_dictionary_encode.rs#prefix_and_dictionary_configuration_replace_one_another`; `tests/upstream_parameter_lifecycle.rs#session_reset_preserves_dictionary_and_parameter_reset_clears_it`; `tests/upstream_parameter_lifecycle.rs#session_reset_preserves_unclaimed_prefix_and_parameter_reset_removes_it`; `tests/upstream_parameter_lifecycle.rs#full_reset_discards_unclaimed_and_claimed_prefixes_before_plain_encoding`; `tests/upstream_parameter_lifecycle.rs#official_codec_decodes_configured_session_and_parameter_reset_frames`. Parameter-only reset clears dictionary handles while preserving Finished/counters; an explicit session reset then enables plain encoding. Full reset also clears unclaimed/claimed prefixes. With declared 10 KiB/100 KiB RDG data, useful dictionary frames differ in size from the cleared frames, which exactly match fresh Rust defaults and decode without a dictionary. Parameter/full resets restore Greedy/checksum-enabled/no-pledge defaults even after Chain/Lazy; C default-level execution and automatic next-frame semantics remain distinct. |
| 3124 | ZSTD_decompressDCtx() with dictionary | partial | `tests/upstream_dictionary.rs#one_shot_dictionary_helper_matches_streaming_and_preserves_shared_handles`. `tests/upstream_dictionary.rs#official_prepared_dictionary_persists_across_an_unsplit_concatenated_input`. `tests/upstream_dictionary.rs#reset_and_reset_with_options_preserve_the_persistent_dictionary`. `tests/upstream_dictionary.rs#setting_or_clearing_a_dictionary_replaces_any_pending_prefix`. `tests/upstream_dictionary.rs#nonzero_frame_id_distinguishes_missing_and_mismatched_dictionaries`. Owned/shared dictionaries persist across frame calls; missing dictionaries fail, and explicit clearing/reset reuse are exercised. Rust reset/reset_with_options intentionally retain dictionaries, so C session-and-parameters reset clearing is represented by explicit set_dictionary(None), not identical API semantics. |
| 3146 | ZSTD_decompressDCtx() with ddict | partial | `tests/upstream_dictionary.rs#one_shot_dictionary_helper_matches_streaming_and_preserves_shared_handles`. `tests/upstream_dictionary.rs#official_prepared_dictionary_persists_across_an_unsplit_concatenated_input`. `tests/upstream_dictionary.rs#reset_and_reset_with_options_preserve_the_persistent_dictionary`. `tests/upstream_dictionary.rs#setting_or_clearing_a_dictionary_replaces_any_pending_prefix`. `tests/upstream_dictionary.rs#nonzero_frame_id_distinguishes_missing_and_mismatched_dictionaries`. Owned/shared dictionaries persist across frame calls; missing dictionaries fail, and explicit clearing/reset reuse are exercised. Rust reset/reset_with_options intentionally retain dictionaries, so C session-and-parameters reset clearing is represented by explicit set_dictionary(None), not identical API semantics. |
| 3166 | ZSTD_decompressDCtx() with prefix | partial | `tests/upstream_dictionary.rs#official_prefix_concatenations_consume_the_prefix_exactly_once`. `tests/upstream_dictionary.rs#prefix_is_consumed_by_one_frame_and_does_not_persist_after_reset`. Raw and formatted owned prefixes decode once and are unavailable to later frames, including after reset. Rust one-shot helpers take persistent dictionaries; the original reusable one-shot DCtx prefix entrypoint is represented through the streaming Decoder. |
| 3181 | ZSTD_fast attach dictionary with hashLog = 25 and chainLog = 25 | blocked | This original dictionary compression, parameter/attachment strategy, context-copy/static workspace or multi-dictionary-cache branch is not ported; shared prepared parsing and ordinary dictionary/prefix encoding and decoding are available. |
| 3208 | ZSTD_dfast attach dictionary with hashLog = 25 and chainLog = 25 | blocked | This original dictionary compression, parameter/attachment strategy, context-copy/static workspace or multi-dictionary-cache branch is not ported; shared prepared parsing and ordinary dictionary/prefix encoding and decoding are available. |
| 3235 | ZSTD_lazy attach dictionary with hashLog = 29 and searchLog = 4 | blocked | This original dictionary compression, parameter/attachment strategy, context-copy/static workspace or multi-dictionary-cache branch is not ported; shared prepared parsing and ordinary dictionary/prefix encoding and decoding are available. |
| 3265 | Dictionary with non-default repcodes | partial | `tests/upstream_dictionary_encode.rs#prepared_dictionaries_with_nondefault_duplicate_repcodes_roundtrip`. `tests/upstream_dictionary.rs#duplicate_initial_repeat_offsets_are_used_and_reset_between_frames`. `tests/upstream_dictionary_encode.rs#official_codec_decodes_raw_prepared_and_omitted_id_dictionary_frames`. Duplicate nondefault10/10/10 prepared dictionaries now encode and decode exact content; an explicit rep2 witness proves decoder initialization. Rust uses its supported match strategy rather than the original level19 optimal-parser setup. |
| 3312 | ZSTD_decompressDCtx() with multiple ddicts | blocked | This original dictionary compression, parameter/attachment strategy, context-copy/static workspace or multi-dictionary-cache branch is not ported; shared prepared parsing and ordinary dictionary/prefix encoding and decoding are available. |
| 3404 | ZDICT_trainFromBuffer_cover | blocked | The original COVER-specific CNBuffer construction/optimization and randomized d/k or four-step search are not implemented. Generic generated-dictionary ID checks run elsewhere but do not replay this strategy path. |
| 3415 | check dictID | blocked | The original COVER-specific CNBuffer construction/optimization and randomized d/k or four-step search are not implemented. Generic generated-dictionary ID checks run elsewhere but do not replay this strategy path. |
| 3420 | ZDICT_optimizeTrainFromBuffer_cover | blocked | The original COVER-specific CNBuffer construction/optimization and randomized d/k or four-step search are not implemented. Generic generated-dictionary ID checks run elsewhere but do not replay this strategy path. |
| 3429 | check dictID | blocked | The original COVER-specific CNBuffer construction/optimization and randomized d/k or four-step search are not implemented. Generic generated-dictionary ID checks run elsewhere but do not replay this strategy path. |
| 3440 | Check input length for magic number | ported | `check_input_length_for_magic_number`. Every source length below four is rejected. |
| 3446 | Check magic Number | ported | `check_magic_number`. Original corrupted first magic byte rejected. |
| 3453 | Content size verification | blocked | Pledged-size validation exists. The original bufferless compressBegin_advanced/compressEnd test pledges 6000 bytes and ends with 5000 CNBuffer bytes at level 1; its exact workload and C error-code assertion are not mapped. |
| 3469 | negative compression level | partial | `tests/upstream_negative_level.rs#negative_level_simple_and_parameter_requests_preserve_the_original_size_assertion`; `tests/upstream_negative_level.rs#both_original_c_negative_level_frames_decode_the_exact_inherited_source`. The exact inherited 1 MiB input is preserved by independently checked source-state replay. Rust level -1 and a fresh parameter request both succeed with equal sizes, additionally equal frame bytes and complete decoded source. Both original C frames decode exactly. The original live advanced CCtx setter/context and caller-owned destination-capacity contract remain unported. |
| 3496 | parameters in order | blocked | The original 5-MiB-buffer/2 input, level-two, enabled long-distance matching and windowLog-18 setter-order permutations require the unimplemented LDM engine and live numeric CCtx setter/configuration semantics. No-dictionary level-two Fast execution is available for this input-size tier, but does not implement these LDM-enabled equal-size and XXH64 output assertions. Detached no-LDM override queries and named-policy dictionary/options ordering are separate coverage. |
| 3513 | parameters disordered | blocked | The original 5-MiB-buffer/2 input, level-two, enabled long-distance matching and windowLog-18 setter-order permutations require the unimplemented LDM engine and live numeric CCtx setter/configuration semantics. No-dictionary level-two Fast execution is available for this input-size tier, but does not implement these LDM-enabled equal-size and XXH64 output assertions. Detached no-LDM override queries and named-policy dictionary/options ordering are separate coverage. |
| 3533 | get dParameter bounds | blocked | Typed DecodeOptions validation and reusable Decoder reset exist; the original C numeric parameter-bound/query/reset contract remains unported. |
| 3539 | wrong dParameter | blocked | Typed DecodeOptions validation and reusable Decoder reset exist; the original C numeric parameter-bound/query/reset contract remains unported. |
| 3548 | out of bound dParameter | blocked | Typed DecodeOptions validation and reusable Decoder reset exist; the original C numeric parameter-bound/query/reset contract remains unported. |
| 3568 | magic-less format test | blocked | Magicless encoding/decoding and its format-selection API absent. |
| 3579 | decompress normally (should fail) | blocked | Magicless encoding/decoding and its format-selection API absent. |
| 3585 | decompress of magic-less frame | blocked | Magicless encoding/decoding and its format-selection API absent. |
| 3608 | empty magic-less format test | blocked | Magicless encoding/decoding and its format-selection API absent. |
| 3619 | decompress of empty magic-less frame | blocked | Magicless encoding/decoding and its format-selection API absent. |
| 3641 | Decompression parameter reset test | blocked | Decoder options and reusable session reset exist, but this original loop over numeric decoder parameter bounds/getters/setters and reset-to-default values is not implemented. The new parameter/full resets are encoder-only and do not reset a decoder caller's output/window resource policy. |
| 3678 | Block compression test | blocked | Public raw-block/bufferless streaming compression and decompression API absent. |
| 3684 | Block decompression test | blocked | Public raw-block/bufferless streaming compression and decompression API absent. |
| 3691 | Huge block streaming compression test | blocked | Public raw-block/bufferless streaming compression and decompression API absent. |
| 3705 | Dictionary Block compression test | blocked | This original dictionary compression, parameter/attachment strategy, context-copy/static workspace or multi-dictionary-cache branch is not ported; shared prepared parsing and ordinary dictionary/prefix encoding and decoding are available. |
| 3716 | Dictionary Block decompression test | blocked | This original dictionary compression, parameter/attachment strategy, context-copy/static workspace or multi-dictionary-cache branch is not ported; shared prepared parsing and ordinary dictionary/prefix encoding and decoding are available. |
| 3733 | Block compression with CDict | blocked | This original dictionary compression, parameter/attachment strategy, context-copy/static workspace or multi-dictionary-cache branch is not ported; shared prepared parsing and ordinary dictionary/prefix encoding and decoding are available. |
| 3749 | RLE detection test | ported | `rle_detection_workload`. Original B/A runs, roundtrip, and <=46-byte compressed-size bound retained. |
| 3764 | ZSTD_generateSequences decode from sequences test | blocked | Public sequence generation/external-sequence compression API absent. |
| 3797 | ZSTD_generateSequences too small output buffer | blocked | Public sequence generation/external-sequence compression API absent. |
| 3822 | ZSTD_getSequences followed by ZSTD_compressSequences | blocked | Public sequence generation/external-sequence compression API absent. |
| 3881 | ZSTD_compressSequencesAndLiterals | blocked | Public sequence generation/external-sequence compression API absent. |
| 3985 | compress %u zeroes | ported | `multiple_blocks_of_zeroes`. Original 1,000,000 zero bytes compressed. |
| 3990 | decompress %u zeroes | ported | `multiple_blocks_of_zeroes`. Original 1,000,000-byte output compared. |
| 3997 | compress %u zeroes | ported | `all_zeroes_issue_137`. Original 100 zero bytes compressed. |
| 4002 | decompress %u zeroes | ported | `all_zeroes_issue_137`. Original 100-byte output compared. |
| 4032 | growing nbSeq | partial | `tests/upstream_fuzzer.rs#growing_sequence_count`. The original counts and triple corpus are retained; this adapter does not exercise the original context-reuse path, and level 19 is unsupported. |
| 4050 | compress lots 3-bytes sequences | adapted | `many_three_byte_sequences`. Exact 131,000-byte seeded triple corpus; level 19 selection absent. |
| 4055 | decompress lots 3-bytes sequence | ported | `many_three_byte_sequences`. Complete triple-corpus output comparison. |
| 4061 | growing literals buffer | partial | `tests/upstream_fuzzer.rs#growing_literals_buffer`. The original literal profile and complete 1..5-MiB schedule run through allocating calls. Reusable contexts exist, but the original context-reuse branch is not exercised by this adapter. |
| 4079 | incompressible data and ill suited dictionary | blocked | Raw/formatted training and named-policy dictionary compression exist. The original 1-MiB CNBuffer training followed by full-buffer byte inversion and 500-byte dictionary compression at level 1 remains unported. No-dictionary level-one Fast is now available; dictionary Fast execution and this exact trained-dictionary/source-state campaign remain the gaps. |
| 4117 | ZSTD_findFrameCompressedSize on skippable frame | blocked | Public compression/decompression bound or frame scanning/skippable metadata API absent. |
| 4121 | ZSTD_getFrameContentSize on skippable frame | blocked | Public compression/decompression bound or frame scanning/skippable metadata API absent. |
| 4125 | ZSTD_getFrameHeader on skippable frame | blocked | Public compression/decompression bound or frame scanning/skippable metadata API absent. |
| 4140 | testing ZSTD error code strings | not-applicable | C numeric error/version/symbol-ABI contract; Rust uses typed Error and Cargo package version. |
| 4149 | testing ZSTD dictionary sizes | blocked | This original dictionary compression, parameter/attachment strategy, context-copy/static workspace or multi-dictionary-cache branch is not ported; shared prepared parsing and ordinary dictionary/prefix encoding and decoding are available. |
| 4172 | testing FSE_normalizeCount() PR#1255 | ported | `tests/upstream/entropy.rs#fuzzer_pr1255_unit_counts_normalize_without_division_by_zero`: exact 32 unit counts at log five avoid arithmetic traps and preserve mass/readback. Original C ignores its generic-error return; Rust accepts this distribution under its independent normalization policy. |
| 4190 | testing FSE_writeNCount() PR#2779 | partial | `tests/upstream/entropy.rs#fuzzer_pr2779_sparse_counts_match_the_exact_nine_byte_header`: exact sparse counts produce C-verified header `20022220022210ea03` with exact readback. C needs destination slack beyond its nine-byte header and rejects the original nine-byte buffer; Rust returns owned output, so the caller-buffer bounds/error branch remains unported. |
| 4206 | testing bitwise intrinsics PR#3045 | not-applicable | C compiler-intrinsic implementation regression; Rust uses language integer operations. |
| 4275 | passing wrong full dict should fail on compressStream2 refPrefix | blocked | This original dictionary compression, parameter/attachment strategy, context-copy/static workspace or multi-dictionary-cache branch is not ported; shared prepared parsing and ordinary dictionary/prefix encoding and decoding are available. |
| 4305 | small dictionary with multithreading and LDM | blocked | This original dictionary compression, parameter/attachment strategy, context-copy/static workspace or multi-dictionary-cache branch is not ported; shared prepared parsing and ordinary dictionary/prefix encoding and decoding are available. |
| 4332 | ZSTD_getCParams() + dictionary | ported | `tests/upstream_parameters.rs#dictionary_sizes_select_the_original_equal_reference_profiles`. All three original fieldwise level-one dictionary-query equalities are preserved for source sizes 16 KiB-1 and 128 KiB-1 and dictionary sizes 400, 10000 and 100000. Value equality replaces C memcmp, and public C source zero maps explicitly to SourceSize::Unknown. |
| 4346 | ZSTD_adjustCParams() + dictionary | ported | `tests/upstream_parameters.rs#dictionary_adjustment_preserves_unknown_profile_and_downsizes_known_input`. The original level-one unknown-source profile is unchanged after adjustment with a 400-byte dictionary, while known source 500 plus dictionary 400 resolves windowLog 10. All fields are compared without C struct-padding assumptions; public C source zero maps explicitly to Unknown. |
| 4357 | check compression mem usage monotonicity over levels for estimateCCtxSize() | blocked | Reusable owned contexts and retained-capacity reporting exist; this original C static-workspace, custom-allocation, automatic-shrink or context-size-estimate contract remains unported. |
| 4373 | check estimateCCtxSize() always larger or equal to ZSTD_estimateCCtxSize_usingCParams() | blocked | Reusable owned contexts and retained-capacity reporting exist; this original C static-workspace, custom-allocation, automatic-shrink or context-size-estimate contract remains unported. |
| 4395 | thread pool API tests | blocked | Multithreaded compressor and shared thread-pool API absent. |
| 4438 | table cleanliness through index reduction | blocked | A bounded reusable match table and retained frame history exist. The original persistent long-index reduction and static-context table-cleanliness campaign remains unported. |
| 4507 | testing ldm no regressions in size for opt parser | blocked | Long-distance matching and optimal parser configuration/size comparison absent. |
| 4544 | testing cdict compression with different attachment strategies | blocked | This original dictionary compression, parameter/attachment strategy, context-copy/static workspace or multi-dictionary-cache branch is not ported; shared prepared parsing and ordinary dictionary/prefix encoding and decoding are available. |

## Fuzzer groups outside the numbered diagnostics

| Upstream function / group | Status and retained obligation |
| --- | --- |
| `FUZ_rand`, `FUZ_randomLength`, `FUZ_rLogLength` | Ported scalar RNG/logarithmic lengths in `support`; independently verified RNG vectors. |
| `RDG_genBuffer`, `RDG_genBlock`, literal distribution | Ported safe owned generator; exact upstream output vectors. Empty Rust input is explicitly handled safely. |
| `FUZ_bug976` | C preprocessor/MIN-macro compile-time regression; no corresponding Rust macro dependency. |
| `FUZ_mallocTests_internal`, `FUZ_mallocTests` | Blocked: custom allocator/context APIs, allocation counters, and macOS malloc-size behavior. |
| `FUZ_decodeSequences` | Blocked: public sequence-export/import test API absent. |
| `threadPoolTests`, `compress_thread` | Blocked: shared thread-pool creation/reuse/simultaneous compression. |
| `fuzzerTests`: simple compression/header/decompression | Adapted in `deterministic_fuzzer_one_shot_cycles`; the original randomized numeric-level and context-selection branches are not exercised by this named-policy adapter. Separate admitted Fast tests are mapped below. |
| `fuzzerTests`: too-small compression destination | Blocked: the one-shot compressor returns an owned vector; its atomic caller-owned destination failure contract is unavailable. Streaming output backpressure is a different contract. |
| `fuzzerTests`: truncated source / too-small decoded destination | Ported via exact truncation/explicit output-limit rejection; safe slice ownership replaces C canaries. |
| `fuzzerTests`: noisy source | Ported bounded mutation/no-panic contract with the upstream mutation algorithm. |
| `fuzzerTests`: bufferless/scattered-segment dictionary compression, decompression and noise | Partial API availability: bounded contexts and dictionary/prefix encoding and decoding exist; the original scattered-segment dictionary compression, context-copy and noise driver remains unported. |
| `fuzzerTests`: `ZSTD_getFrameHeader`, decompressed-size scan | Header content-size assertion ported; general concatenation-size scanner absent. |
| `findDiff`, display/CLI/clock and seed parsing | Rust assertion messages, test harness, and explicit deterministic seed/cycle controls replace process-exit and logging glue. |

## Small upstream programs

| Source / entry point | Status and executable mapping |
| --- | --- |
| `roundTripCrash.c::roundTripTest`, `roundTripCheck`, `checkBuffers` | Adapted in `round_trip_crash_corpora`: exact length/byte equality across fifty deterministic corpora. The original XXH32-derived full-level selection is not executed by this adapter; separate Fast-only level APIs do not port that campaign. |
| `roundTripCrash.c::cctxParamRoundTripTest` | Reusable streaming contexts exist; the original parameter-object setup, two-worker compression and overlap-control schedule remain unported. |
| `roundTripCrash.c::loadFile`, `fileCheck`, `main` | C command-line/file diagnostics are test-driver glue; Rust integration harness supplies deterministic slices. No user-file reader is claimed. |
| `longmatch.c::main`, `compress` | Only its 256-KiB source layout is adapted in `long_match_input_layout`. The original continuous 8-GiB campaign, randomized flush boundaries, numeric level/strategy controls and index rollover remain unported; bounded streaming and independent named Rust policies exist. A smaller one-shot test is not an index-overflow port. |
| `invalidDictionaries.c::main` | Ported: the exact original bytes fail shared Auto/Formatted dictionary preparation. Both Rust encoding and decoding require this same validated Dictionary type; there is no second unchecked formatted constructor. |
| `largeDictionary.c::main`, `compress` | Blocked: original 2-GiB buffer, 1-GiB chunks, 31-bit window, worker/optimal-parser configuration and continuous stream history. The code executes ten continued chunks plus one final chunk; its comment says 30 GB. |
| `legacy.c::testSimpleAPI` | Blocked: successful decoding of the embedded historical format versions is required; rejecting their magic does not satisfy the test. |
| `legacy.c::testStreamingAPI` | Blocked: historical frame decoding and streaming progress/reset contracts. |
| `legacy.c::testFrameDecoding` | Blocked: historical frame boundary scans and decompression bounds. |

This is a complete inventory of these sources, not a claim that all upstream
capabilities or tests are already executable in the Rust codec.

## Module 12 admitted whole-buffer Fast execution

The numeric API now executes admitted no-dictionary Fast profiles. This changes
availability, not the coverage attributed to older named-policy adapters.
Historical Module 8/10/11 evidence above describes those original snapshots.

`tests/upstream_negative_level.rs` preserves the original inherited one-MiB
source at :3469, adapts successful simple/requested size equality, and decodes
both complete original C frames. `tests/upstream_reference_fast.rs` preserves
the original :1554 empty-input and :1637 30-byte level ranges, with explicit
supported/unsupported counts, and the exact simple-round-trip selector schedule.
The separate 92-recipe C/Rust campaign validates complete frames in both
directions. Details and immutable evidence are in
[`reference-fast-validation.md`](../../docs/reference-fast-validation.md).

All these source units remain partial. Live numeric streaming/context setters,
caller-buffer failure behavior, dictionary Fast, LDM, DoubleFast, row/tree
algorithms, workers and CLI level behavior remain separate obligations. No test
is promoted merely because its numeric level is now admitted by another API.
