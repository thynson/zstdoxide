# Persistent dictionary in-place output measurements

The frozen workload compares suffix-input in-place decompression with direct
caller-owned output under identical persistent dictionary selection. Eleven cases
cover raw dictionary distances/crossings, prepared IDs and omitted IDs, treeless
literals and four streams, repeated sequence tables, HTTP, an empty frame,
concatenated dictionary frames and a 120,000-byte raw block with actual overlap.
The native oracle is never part of a measured decoder.

## Method and limits

Twenty-three independently retained primary executables cover safe, all, and
21 isolated unsafe features. Safe and all are also rebuilt independently and
measured in a separate repeat. Each backend has 220 raw timing rows: 11 cases,
four modes and five samples. Every timed operation takes its own clock reading,
including the direct baseline; timer overhead is retained, not subtracted.
The iteration count is deterministic and bounded from 8 to 1,024.

`in-place-decode` restores the original encoded suffix before starting the clock.
`restore-only` times that copy separately; `in-place-with-restore` includes the
copy and decoding together. `direct-decode` uses a separate immutable encoded
slice. Caller buffer allocation, fixture preparation, output assertions and
allocation-ledger bookkeeping are excluded. Decoder scratch allocation/drop,
frame validation and checksums are included. Dictionary preparation and shared-owner construction occur before timing.
The workload does not measure prefixes or incremental overlap.

Two fresh process inventories before primary and repeat showed no matched task
compiler/test/build/archive jobs. This does not establish whole-desktop idleness.
Fixed backend order and five samples do not establish statistical significance,
architecture independence or a universal unsafe speedup. Ratios below use equal
weights across the ten nonempty cases and the geometric mean of each case's
median latency ratio. Lower ratios mean lower observed latency. Empty output
remains in the raw data, outside the aggregate.

## Observed latency ratios

| Campaign | Backend | Decode / direct | Restore + decode / direct | Decode / safe decode | Restore + decode / safe combined |
| --- | --- | ---: | ---: | ---: | ---: |
| primary | safe | 1.0027 | 1.1281 | 1.0000 | 1.0000 |
| primary | unsafe-bitstream | 1.0075 | 1.1134 | 1.0165 | 0.9984 |
| primary | unsafe-byte-cursor | 1.0114 | 1.0926 | 1.0415 | 1.0001 |
| primary | unsafe-dictionary | 1.0030 | 1.0948 | 1.0142 | 0.9840 |
| primary | unsafe-fse | 1.0122 | 1.1132 | 1.0011 | 0.9786 |
| primary | unsafe-fse-encoder | 1.0468 | 1.1627 | 1.0204 | 1.0074 |
| primary | unsafe-history | 1.0001 | 1.0870 | 1.0002 | 0.9662 |
| primary | unsafe-huffman | 0.9993 | 1.1098 | 1.0032 | 0.9902 |
| primary | unsafe-huffman-codebook | 1.0113 | 1.1155 | 1.0271 | 1.0070 |
| primary | unsafe-in-place-buffer | 1.0091 | 1.0971 | 1.0044 | 0.9706 |
| primary | unsafe-match-chain | 1.0088 | 1.1043 | 1.0020 | 0.9749 |
| primary | unsafe-match-table | 1.0126 | 1.1303 | 1.0075 | 0.9996 |
| primary | unsafe-optimal-frequencies | 1.0126 | 1.1121 | 1.0186 | 0.9944 |
| primary | unsafe-optimal-matches | 1.0087 | 1.1157 | 1.0077 | 0.9906 |
| primary | unsafe-optimal-paths | 1.0029 | 1.1114 | 1.0104 | 0.9952 |
| primary | unsafe-optimal-short-key | 1.0016 | 1.1034 | 1.0152 | 0.9940 |
| primary | unsafe-reference-chain | 1.0073 | 1.1147 | 0.9968 | 0.9804 |
| primary | unsafe-reference-tree | 0.9994 | 1.0989 | 1.0095 | 0.9866 |
| primary | unsafe-ring-history | 1.0144 | 1.1203 | 1.0077 | 0.9892 |
| primary | unsafe-row-table | 1.0120 | 1.1137 | 1.0213 | 0.9990 |
| primary | unsafe-slice-history | 1.0187 | 1.1111 | 0.9941 | 0.9637 |
| primary | unsafe-training | 0.9997 | 1.1113 | 1.0032 | 0.9912 |
| primary | all | 1.0237 | 1.1088 | 0.9969 | 0.9597 |
| repeat | safe | 1.0146 | 1.1126 | 1.0000 | 1.0000 |
| repeat | all | 0.9976 | 1.1252 | 1.0237 | 1.0530 |

## Allocation and retained evidence

Clock-free qualification checks full output, protected tails and guards, then
records separate in-place/direct scratch and dictionary ledgers for each case.
Dictionary construction uses an independent tracking allocator; its snapshot is
unchanged by decoding, then all dictionary and scratch storage is released.
All 44 ledger rows per executable match across 23 primary and both repeat builds.
The in-place destination keeps caller ownership; the decoder does not copy the
whole input, allocate output, clone the selected handle or retain a copied
dictionary/history buffer. The restoration
copy belongs to the benchmark's repeated-work setup and is accounted explicitly.

The archive retains measurement source, deterministic fixture identities and
payloads, all executable hashes, actual executables, allocation ledgers, every
raw sample, summaries, process observer source and both CPU barriers. The
benchmark snapshot adds only its bench registration and harness to identical
production source. Final test snapshots and standalone earlier components keep
their own identities. Evidence: `evidence/2026-10-27-dictionary-in-place-benchmarks.json`.
