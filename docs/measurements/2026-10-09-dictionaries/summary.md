# Backend measurements

All fifteen cases produced identical deterministic fingerprints in every build.
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
| Compression input | 99328 | 1339 | 1.3481% | `b6ec9854a7bed506` |
| Official text decompression fixture | 370326 | 7124 | 1.9237% | `4b678f26bb5708b9` |
| Streaming compression input | 99328 | 1336 | 1.3450% | `5bc47910249df5af` |
| Streaming official text fixture | 370326 | 7124 | 1.9237% | `4b678f26bb5708b9` |
| Formatted-dictionary compression | 32768 | 27 | 0.0824% | `186ac1df4a251e5f` |
| Prepared HTTP dictionary decoding | 76096 | 27328 | 35.9125% | `05fdf95c5c74f365` |

| Feature | Target workload | Safe ns/unit | Feature ns/unit | Relative throughput |
| --- | --- | ---: | ---: | ---: |
| `unsafe-byte-cursor` | byte_cursor | 0.5235 | 0.4981 | 1.051x |
| `unsafe-bitstream` | reverse_bits | 1.4032 | 1.4009 | 1.002x |
| `unsafe-fse` | fse_lookup | 0.7540 | 0.7590 | 0.993x |
| `unsafe-huffman` | huffman_decode | 6.1444 | 6.1476 | 0.999x |
| `unsafe-history` | history_overlap | 0.6282 | 0.3782 | 1.661x |
| `unsafe-ring-history` | ring_overlap | 0.9496 | 0.7736 | 1.228x |
| `unsafe-match-table` | match_table | 1.0213 | 1.0164 | 1.005x |
| `unsafe-dictionary` | dictionary_content | 1.0205 | 1.0064 | 1.014x |
| `unsafe-training` | raw_training | 9.3851 | 9.4417 | 0.994x |

| Workload | Safe ns/unit | All features ns/unit | Relative throughput |
| --- | ---: | ---: | ---: |
| byte_cursor | 0.5235 | 0.5123 | 1.022x |
| reverse_bits | 1.4032 | 1.4310 | 0.981x |
| fse_lookup | 0.7540 | 0.7552 | 0.998x |
| huffman_decode | 6.1444 | 6.4481 | 0.953x |
| history_overlap | 0.6282 | 0.3810 | 1.649x |
| ring_overlap | 0.9496 | 0.7721 | 1.230x |
| match_table | 1.0213 | 1.0245 | 0.997x |
| compress | 2.8833 | 2.6113 | 1.104x |
| decompress | 1.2101 | 0.9448 | 1.281x |
| stream_decompress | 2.3127 | 2.3953 | 0.966x |
| stream_compress | 2.7635 | 2.5278 | 1.093x |
| dictionary_content | 1.0205 | 1.0029 | 1.018x |
| dictionary_compress | 3.4845 | 3.3110 | 1.052x |
| dictionary_decompress | 4.3854 | 4.4677 | 0.982x |
| raw_training | 9.3851 | 9.4979 | 0.988x |

Retained memory is the maximum context-reported heap capacity observed after process
calls. It excludes inline state, caller buffers, allocator overhead, and transient scratch.
These checks run outside timed regions and verify exact output bytes. Equal capacity
for the longer run establishes a plateau for this workload, not a universal memory limit.

| Context | Short workload | Long workload | Short retained bytes | Long retained bytes |
| --- | --- | --- | ---: | ---: |
| Decoder | 4 concatenated frames | 64 concatenated frames | 509902 | 509902 |
| Decoder, one frame | 524288 decoded bytes | 8388608 decoded bytes | 264107 | 264107 |
| Encoder | 4 input blocks | 64 input blocks, one frame | 1483361 | 1483361 |

The decoder delivered 23700864 bytes in the long memory trial.
The encoder accepted 8388608 bytes in 131072-byte blocks.
Streaming input/output chunks are 257/4096 bytes for decoding and 4096/1024 bytes for encoding.
After reset, the timed decoder retains 504270 bytes and the timed encoder 1324641 bytes.
With persistent prepared dictionaries, decoder/encoder reset retains 17099/1525990 bytes respectively, including each selected dictionary once per context report.
Raw training admits at most 65536 bytes from 256 samples (131072 input bytes), with 64-byte segments and a 2048-byte output cap.
The selected raw dictionary contains 2048 bytes with full-output FNV-1a `b3a3cc2c22959aae`. Training timings include scratch allocation and the complete selection algorithm.

Raw sample ranges and fingerprints are recorded in the adjacent TSV files.
Compiler, machine, and timestamp are recorded in `environment.txt`.
The benchmark prints its seed, sample count, and duration in each TSV file.
