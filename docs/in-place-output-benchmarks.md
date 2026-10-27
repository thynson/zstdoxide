# Bounded in-place output measurements

The frozen workload compares suffix-input in-place decompression with direct
caller-owned output on identical encoded bytes and output allowances. Eleven
cases include generated mixed/raw/RLE/compressed blocks, small blocks, larger
content, HTTP, an entropy-heavy fixture, empty output, concatenation and a
120,000-byte raw block whose source and destination actually overlap at start 0.
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
frame validation and checksums are included. The dictionary-free workload does
not measure persistent dictionaries, prefixes or incremental overlap.

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
| primary | safe | 1.0670 | 1.1845 | 1.0000 | 1.0000 |
| primary | unsafe-bitstream | 0.9323 | 1.0541 | 0.8071 | 0.8220 |
| primary | unsafe-byte-cursor | 1.0076 | 1.1201 | 0.8073 | 0.8085 |
| primary | unsafe-dictionary | 0.9594 | 1.0410 | 0.9125 | 0.8919 |
| primary | unsafe-fse | 0.9820 | 1.1228 | 0.8218 | 0.8464 |
| primary | unsafe-fse-encoder | 0.9947 | 1.0944 | 0.8774 | 0.8696 |
| primary | unsafe-history | 0.9509 | 1.0532 | 0.8885 | 0.8864 |
| primary | unsafe-huffman | 0.9876 | 1.1437 | 0.8486 | 0.8852 |
| primary | unsafe-huffman-codebook | 1.0006 | 1.0982 | 0.8392 | 0.8296 |
| primary | unsafe-in-place-buffer | 1.0029 | 1.1004 | 0.8217 | 0.8122 |
| primary | unsafe-match-chain | 0.9801 | 1.0875 | 0.8043 | 0.8039 |
| primary | unsafe-match-table | 0.9771 | 1.0790 | 0.8125 | 0.8082 |
| primary | unsafe-optimal-frequencies | 0.9876 | 1.1058 | 0.8023 | 0.8092 |
| primary | unsafe-optimal-matches | 0.9793 | 1.0920 | 0.8111 | 0.8147 |
| primary | unsafe-optimal-paths | 0.9935 | 1.0942 | 0.8187 | 0.8122 |
| primary | unsafe-optimal-short-key | 0.9890 | 1.0910 | 0.8139 | 0.8088 |
| primary | unsafe-reference-chain | 0.9816 | 1.0940 | 0.8143 | 0.8176 |
| primary | unsafe-reference-tree | 0.9998 | 1.1015 | 0.8271 | 0.8208 |
| primary | unsafe-ring-history | 0.9853 | 1.1138 | 0.8098 | 0.8246 |
| primary | unsafe-row-table | 0.9687 | 1.0841 | 0.7950 | 0.8014 |
| primary | unsafe-slice-history | 0.9899 | 1.1072 | 0.8109 | 0.8170 |
| primary | unsafe-training | 1.0055 | 1.0956 | 0.8203 | 0.8051 |
| primary | all | 0.9914 | 1.1054 | 0.8041 | 0.8076 |
| repeat | safe | 0.9979 | 1.1048 | 1.0000 | 1.0000 |
| repeat | all | 0.9910 | 1.0895 | 1.0056 | 0.9985 |

## Allocation and retained evidence

Clock-free qualification checks full output, protected tails and guards, then
records separate in-place/direct scratch ledgers for each case. All 23 primary
and both repeat ledgers are byte-identical, and all scratch storage is released.
The in-place destination keeps caller ownership; the decoder does not copy the
whole input, allocate output, or retain a separate history buffer. The restoration
copy belongs to the benchmark's repeated-work setup and is accounted explicitly.

The archive retains measurement source, deterministic fixture identities and
payloads, all executable hashes, actual executables, allocation ledgers, every
raw sample, summaries, process observer source and both CPU barriers. The
benchmark snapshot adds only its bench registration and harness to identical
production source. Final test snapshots and standalone earlier components keep
their own identities. Evidence: `evidence/2026-10-27-in-place-benchmarks.json`.
