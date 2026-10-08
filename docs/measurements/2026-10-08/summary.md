# Backend measurements

All eight cases produced identical deterministic fingerprints in every build.
Whole-codec output sizes and full-output FNV-1a checksums also match across builds.
These noncryptographic checksums supplement full-output interoperability tests.
Each table entry is the median of the configured timed samples. Higher relative
throughput is better; 1.00 means equal to the safe build. Workstation noise and
allocation/sampled-fingerprint overhead are included; these are not universal speedups.
Full-output checksums and the following sizes are measured outside the timed regions.

| Whole-codec workload | Original/decoded bytes | Compressed bytes | Compressed/original | Output FNV-1a |
| --- | ---: | ---: | ---: | --- |
| Compression input | 99328 | 1339 | 1.3481% | `b6ec9854a7bed506` |
| Official text decompression fixture | 370326 | 7124 | 1.9237% | `4b678f26bb5708b9` |

| Feature | Target workload | Safe ns/unit | Feature ns/unit | Relative throughput |
| --- | --- | ---: | ---: | ---: |
| `unsafe-byte-cursor` | byte_cursor | 0.4924 | 0.5149 | 0.956x |
| `unsafe-bitstream` | reverse_bits | 1.3950 | 1.4308 | 0.975x |
| `unsafe-fse` | fse_lookup | 0.7471 | 0.7491 | 0.997x |
| `unsafe-huffman` | huffman_decode | 6.0675 | 6.2373 | 0.973x |
| `unsafe-history` | history_overlap | 0.6258 | 0.3801 | 1.646x |
| `unsafe-match-table` | match_table | 1.0374 | 1.0119 | 1.025x |

| Workload | Safe ns/unit | All features ns/unit | Relative throughput |
| --- | ---: | ---: | ---: |
| byte_cursor | 0.4924 | 0.5011 | 0.983x |
| reverse_bits | 1.3950 | 1.3943 | 1.001x |
| fse_lookup | 0.7471 | 0.7485 | 0.998x |
| huffman_decode | 6.0675 | 6.2585 | 0.969x |
| history_overlap | 0.6258 | 0.3746 | 1.671x |
| match_table | 1.0374 | 1.0338 | 1.003x |
| compress | 1.7035 | 1.7200 | 0.990x |
| decompress | 1.1925 | 0.9136 | 1.305x |

Raw sample ranges and fingerprints are recorded in the adjacent TSV files.
Compiler, machine, and timestamp are recorded in `environment.txt`.
The benchmark prints its seed, sample count, and duration in each TSV file.

This run includes timing outliers: the all-features compression samples ranged
from 1.6674 to 2.5146 ns/byte, and the FSE lookup under `unsafe-huffman` ranged
from 0.7472 to 1.1698 ns/transition. All samples remain represented in the raw
ranges. Their medians do not establish causal effects of unrelated features.
Small backend differences should be treated as uncertain in this single
sequential workstation run. The history-copy change was materially larger:
1.646x isolated and 1.671x with all features. Whole-codec all-features
decompression was 1.305x the safe throughput on this text fixture; compression
was effectively unchanged at 0.990x. These results apply to the stated inputs.
