# Backend measurements

All 24 cases produced identical deterministic fingerprints in every build.
Whole-codec output sizes and full-output FNV-1a checksums also match across builds.
These noncryptographic checksums supplement full-output interoperability tests.
Each table entry is the median of the configured timed samples. Higher relative
throughput is better; 1.00 means equal to the safe build. Workstation noise and
allocation/sampled-fingerprint overhead are included; these are not universal speedups.
Full-output checksums and the following sizes are measured outside the timed regions.
Streaming timings reuse contexts and caller buffers, including reset and progress checks;
one-shot timings include allocation. Their difference is not a backend-only speedup.

| Whole-codec workload | Original/decoded bytes | Compressed bytes | Compressed/original | Output FNV-1a |
| --- | ---: | ---: | ---: | --- |
| Compression input | 99328 | 1201 | 1.2091% | `9b3876e0de9e8502` |
| Official text decompression fixture | 370326 | 7124 | 1.9237% | `4b678f26bb5708b9` |
| Streaming compression input | 99328 | 1198 | 1.2061% | `e31ed9f84208ca27` |
| Streaming official text fixture | 370326 | 7124 | 1.9237% | `4b678f26bb5708b9` |
| Formatted-dictionary compression | 32768 | 27 | 0.0824% | `186ac1df4a251e5f` |
| Prepared HTTP dictionary decoding | 76096 | 27328 | 35.9125% | `05fdf95c5c74f365` |

| Feature | Target workload | Safe ns/unit | Feature ns/unit | Relative throughput |
| --- | --- | ---: | ---: | ---: |
| `unsafe-byte-cursor` | byte_cursor | 0.5115 | 0.4975 | 1.028x |
| `unsafe-bitstream` | reverse_bits | 1.3918 | 1.3940 | 0.998x |
| `unsafe-fse` | fse_lookup | 0.7497 | 0.7490 | 1.001x |
| `unsafe-huffman` | huffman_decode | 6.3061 | 6.1479 | 1.026x |
| `unsafe-history` | history_overlap | 0.6226 | 0.3745 | 1.662x |
| `unsafe-ring-history` | ring_overlap | 0.9435 | 0.7804 | 1.209x |
| `unsafe-match-table` | match_table | 1.0263 | 1.0265 | 1.000x |
| `unsafe-dictionary` | dictionary_content | 0.9924 | 1.0220 | 0.971x |
| `unsafe-training` | raw_training | 9.4301 | 10.0052 | 0.943x |
| `unsafe-fse-encoder` | fse_encode_lookup | 1.1416 | 1.2306 | 0.928x |
| `unsafe-huffman-codebook` | huffman_codebook | 0.7454 | 0.7618 | 0.978x |
| `unsafe-match-chain` | match_chain | 2.3919 | 2.3657 | 1.011x |

| Workload | Safe ns/unit | All features ns/unit | Relative throughput |
| --- | ---: | ---: | ---: |
| byte_cursor | 0.5115 | 0.4929 | 1.038x |
| reverse_bits | 1.3918 | 1.4957 | 0.931x |
| fse_lookup | 0.7497 | 0.7573 | 0.990x |
| huffman_decode | 6.3061 | 6.2001 | 1.017x |
| history_overlap | 0.6226 | 0.3787 | 1.644x |
| ring_overlap | 0.9435 | 0.8156 | 1.157x |
| match_table | 1.0263 | 1.0902 | 0.941x |
| compress | 3.1279 | 3.1513 | 0.993x |
| decompress | 1.1892 | 0.9460 | 1.257x |
| stream_decompress | 2.2991 | 2.2425 | 1.025x |
| stream_compress | 3.0189 | 2.9297 | 1.030x |
| dictionary_content | 0.9924 | 1.0211 | 0.972x |
| dictionary_compress | 4.9111 | 5.6150 | 0.875x |
| dictionary_decompress | 4.3462 | 4.5612 | 0.953x |
| raw_training | 9.4301 | 9.4442 | 0.999x |
| fse_encode_lookup | 1.1416 | 1.2477 | 0.915x |
| huffman_codebook | 0.7454 | 0.7549 | 0.987x |
| match_chain | 2.3919 | 1.8469 | 1.295x |
| strategy_greedy_compressible | 3.0672 | 2.9703 | 1.033x |
| strategy_greedy_ambiguous | 11.5277 | 12.0150 | 0.959x |
| strategy_chain_compressible | 4.4887 | 4.6590 | 0.963x |
| strategy_chain_ambiguous | 24.9433 | 26.4940 | 0.941x |
| strategy_lazy_compressible | 6.0918 | 5.2536 | 1.160x |
| strategy_lazy_ambiguous | 21.6123 | 21.6593 | 0.998x |

The named strategies receive identical source bytes within each source group.
The ambiguous source uses one identical prepared raw dictionary; preparation is
outside timing, while reset, reused context work, and sampled hashes are included.
The compressible source has no dictionary. Strategies may produce different frames;
each strategy's frame bytes, sizes, and context capacities match across all backends.
These independent Rust policies do not represent upstream compression levels.

| Strategy | Source | Original bytes | Dictionary bytes | Compressed bytes | Compressed/original | Reset retained bytes | Output FNV-1a |
| --- | --- | ---: | ---: | ---: | ---: | ---: | --- |
| greedy | compressible | 99328 | 0 | 1198 | 1.2061% | 1351457 | `e31ed9f84208ca27` |
| greedy | ambiguous | 8320 | 8960 | 403 | 4.8438% | 1425572 | `9300b81f268a40ac` |
| chain | compressible | 99328 | 0 | 1203 | 1.2111% | 10259371 | `54ae85f06795b9e7` |
| chain | ambiguous | 8320 | 8960 | 369 | 4.4351% | 12959340 | `709e079870940354` |
| lazy | compressible | 99328 | 0 | 1206 | 1.2142% | 10266865 | `c7641d7dc777fba6` |
| lazy | ambiguous | 8320 | 8960 | 279 | 3.3534% | 12942794 | `e97f9d2a00cac577` |

Inverse-FSE, Huffman-codebook and chain-candidate setup is outside timing.
Complete inverse relations, canonical ranks and candidate coordinates are checked
before timing; all result records have full byte hashes in the adjacent TSV files.
The chain lookup workload has sixteen candidates per key and a sixteen-visit cap;
its retained-capacity field counts only owned chain storage, excluding input and result vectors.

Retained memory is the maximum context-reported heap capacity observed after process
calls. It excludes inline state, caller buffers, allocator overhead, and transient scratch.
These checks run outside timed regions and verify exact output bytes. Equal capacity
for the longer run establishes a plateau for this workload, not a universal memory limit.

| Context | Short workload | Long workload | Short retained bytes | Long retained bytes |
| --- | --- | --- | ---: | ---: |
| Decoder | 4 concatenated frames | 64 concatenated frames | 509902 | 509902 |
| Decoder, one frame | 524288 decoded bytes | 8388608 decoded bytes | 263841 | 263841 |
| Encoder | 4 input blocks | 64 input blocks, one frame | 1510177 | 1510177 |

The decoder delivered 23700864 bytes in the long memory trial.
The encoder accepted 8388608 bytes in 131072-byte blocks.
Streaming input/output chunks are 257/4096 bytes for decoding and 4096/1024 bytes for encoding.
After reset, the timed decoder retains 504270 bytes and the timed encoder 1351457 bytes.
With persistent prepared dictionaries, decoder/encoder reset retains 17099/1827319 bytes respectively, including each selected dictionary once per context report.
Raw training admits at most 65536 bytes from 256 samples (131072 input bytes), with 64-byte segments and a 2048-byte output cap.
The selected raw dictionary contains 2048 bytes with full-output FNV-1a `b3a3cc2c22959aae`. Training timings include scratch allocation and the complete selection algorithm.

Raw sample ranges and fingerprints are recorded in the adjacent TSV files.
Compiler, machine, and timestamp are recorded in `environment.txt`.
The benchmark prints its seed, sample count, and duration in each TSV file.
