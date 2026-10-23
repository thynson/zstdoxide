# Retained Ultra2 complete-frame measurements

The 2026-10-23 campaign measures complete Global one-shot compression plus
output destruction on the 26 preselected independently decoded cases. Allocation
surveys and frame validation run outside the clocks. Primary safe, each of 19
isolated unsafe features and all-features builds contribute 546 case rows. An
independently rebuilt safe/all repeat contributes 52 more: 598 rows and 2,990
actual samples in total. All 23 executed binaries, frames, sources and samples
are retained. Both campaigns passed complete source and exact-frame validation.

Host: aarch64-apple-darwin, macOS 15.7.7; pinned nightly-2026-10-08. Completed
task-owned CPU barriers precede each campaign. They establish completion of our
build/test/archive jobs, not OS idleness or freedom from unrelated desktop work.

## Observed cost ratios

Ratios compare median compression-plus-drop cost per byte with safe on the
same source in the same campaign. Less than one is faster. Geometric means
weight the 26 cases equally; the ranges below are across cases, not confidence
intervals.

| Campaign | Backend | Geometric mean | Minimum case | Maximum case |
| --- | --- | ---: | ---: | ---: |
| primary | safe | 1.0000 | 1.0000 | 1.0000 |
| primary | unsafe-byte-cursor | 1.0340 | 0.9330 | 1.1497 |
| primary | unsafe-bitstream | 1.0031 | 0.9324 | 1.0637 |
| primary | unsafe-history | 1.0055 | 0.9377 | 1.0767 |
| primary | unsafe-ring-history | 1.0063 | 0.9358 | 1.0659 |
| primary | unsafe-fse | 1.0039 | 0.9355 | 1.0611 |
| primary | unsafe-fse-encoder | 1.0937 | 0.9838 | 1.4076 |
| primary | unsafe-huffman | 1.1300 | 1.0481 | 1.1784 |
| primary | unsafe-huffman-codebook | 1.1076 | 0.9846 | 1.2331 |
| primary | unsafe-match-table | 1.1270 | 1.0417 | 1.2485 |
| primary | unsafe-match-chain | 1.1087 | 1.0072 | 1.2103 |
| primary | unsafe-dictionary | 1.0939 | 1.0218 | 1.1665 |
| primary | unsafe-training | 1.0458 | 0.9812 | 1.1360 |
| primary | unsafe-reference-chain | 1.1485 | 0.9851 | 1.7975 |
| primary | unsafe-row-table | 1.0076 | 0.9362 | 1.0635 |
| primary | unsafe-reference-tree | 1.0686 | 0.9505 | 1.7187 |
| primary | unsafe-optimal-frequencies | 1.0095 | 0.9377 | 1.0730 |
| primary | unsafe-optimal-paths | 1.1329 | 0.9651 | 1.9664 |
| primary | unsafe-optimal-matches | 1.0114 | 0.9434 | 1.1034 |
| primary | unsafe-optimal-short-key | 1.0067 | 0.9404 | 1.0664 |
| primary | all | 0.9913 | 0.9316 | 1.0576 |
| repeat | safe | 1.0000 | 1.0000 | 1.0000 |
| repeat | all | 1.3041 | 0.9577 | 1.8630 |

The all/safe aggregate changes from 0.9913 in the primary run to 1.3041 in
the independent repeat. These observations do not establish a stable unsafe
speedup. Fixed execution order, thermal state, compiler/code placement and
unrelated desktop activity limit interpretation. Features not used by a given
compression path can still show timing variation; do not attribute every ratio
to that feature's algorithm. Keep the safe default.

## Evidence

Normal and optimized timed evidence controls each pass 33 checks: 31 effective
forgeries are rejected and two positives accepted, with zero child attempts.
The actual barriers bind the completed 22-product/21-no_std matrix, final native
and foreign-platform gates, all 1,672 native public requests, and 21 clock-free
backend validations. No synthetic control is represented as an actual run.

Archive: `docs/evidence/2026-10-23-ultra2-retained-frames-timed.tar.gz`.
SHA-256: `de057faae38b5cd742b713cdea752adedbe1140b115eb69336ebd39135bb27d8`.
The companion JSON receipt and independent lossless reconstruction bind every
bundle file. Only individually hashed redundant Cargo build intermediates were
omitted when preparing the bundle; actual measured executables remain embedded.
The archive includes per-case CSV and raw sample records for both campaigns.
