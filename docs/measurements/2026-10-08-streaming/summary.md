# Backend measurements

All eleven cases produced identical deterministic fingerprints in every build.
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

| Feature | Target workload | Safe ns/unit | Feature ns/unit | Relative throughput |
| --- | --- | ---: | ---: | ---: |
| `unsafe-byte-cursor` | byte_cursor | 0.5286 | 0.5290 | 0.999x |
| `unsafe-bitstream` | reverse_bits | 1.3929 | 1.4331 | 0.972x |
| `unsafe-fse` | fse_lookup | 0.7444 | 0.7680 | 0.969x |
| `unsafe-huffman` | huffman_decode | 6.2346 | 6.1817 | 1.009x |
| `unsafe-history` | history_overlap | 0.6279 | 0.3859 | 1.627x |
| `unsafe-ring-history` | ring_overlap | 0.9486 | 1.0033 | 0.945x |
| `unsafe-match-table` | match_table | 1.0300 | 1.0192 | 1.011x |

| Workload | Safe ns/unit | All features ns/unit | Relative throughput |
| --- | ---: | ---: | ---: |
| byte_cursor | 0.5286 | 0.5099 | 1.037x |
| reverse_bits | 1.3929 | 1.3963 | 0.998x |
| fse_lookup | 0.7444 | 0.7508 | 0.991x |
| huffman_decode | 6.2346 | 6.1988 | 1.006x |
| history_overlap | 0.6279 | 0.3782 | 1.660x |
| ring_overlap | 0.9486 | 0.7690 | 1.234x |
| match_table | 1.0300 | 1.0158 | 1.014x |
| compress | 1.8637 | 1.7065 | 1.092x |
| decompress | 1.1959 | 0.9158 | 1.306x |
| stream_decompress | 1.6338 | 1.4634 | 1.116x |
| stream_compress | 1.7546 | 1.6325 | 1.075x |

Retained memory is the maximum context-reported heap capacity observed after process
calls. It excludes inline state, caller buffers, allocator overhead, and transient scratch.
These checks run outside timed regions and verify exact output bytes. Equal capacity
for the longer run establishes a plateau for this workload, not a universal memory limit.

| Context | Short workload | Long workload | Short retained bytes | Long retained bytes |
| --- | --- | --- | ---: | ---: |
| Decoder | 4 concatenated frames | 64 concatenated frames | 509902 | 509902 |
| Decoder, one frame | 524288 decoded bytes | 8388608 decoded bytes | 264107 | 264107 |
| Encoder | 4 input blocks | 64 input blocks, one frame | 1352289 | 1352289 |

The decoder delivered 23700864 bytes in the long memory trial.
The encoder accepted 8388608 bytes in 131072-byte blocks.
Streaming input/output chunks are 257/4096 bytes for decoding and 4096/1024 bytes for encoding.
After reset, the timed decoder retains 504270 bytes and the timed encoder 1193569 bytes.

Raw sample ranges and fingerprints are recorded in the adjacent TSV files.
Compiler, machine, and timestamp are recorded in `environment.txt`.
The benchmark prints its seed, sample count, and duration in each TSV file.
