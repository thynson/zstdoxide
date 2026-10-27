# Compression-bound and skippable-write measurements

Eleven deterministic cases cover empty/one-byte payloads, small-margin and block
boundaries, scalar width limits, invalid variants and short destinations. Scalar
source_size and writer payload/capacity/variant are separate recorded fields:
width-limit arithmetic uses small genuine payloads, never gigantic allocations.

## Method and limits

Twenty-three primary executables cover safe, all and 21 isolated unsafe features.
Independent safe/all rebuilds provide a separate repeat. Each executable yields
110 raw samples: eleven cases, two operations and five samples with deterministic
128..1,024 iterations. One clock brackets the complete batch; loop/dispatch/
black-box overhead is retained. Tiny scalar work can be dominated by this overhead, so do not interpret
small latency differences as a general arithmetic or unsafe performance benefit.
The utility implementations share the same safe scalar/slice-copy representation
in every configuration; other codec features are still compiled independently.

Caller storage, payload construction, hashes and guard/result assertions occur
outside timed calls. Writing includes variant/size/capacity validation, header
serialization and payload copying; repeated writes are idempotent in the same
caller buffer. No source restoration is necessary or excluded from a changing
state. Bounds and writes have different contracts and are not interchangeable.
All/safe ratios compare identical operations and cases. Geometric means weight
the ten nonempty-payload cases equally; empty payload remains in raw data.

Two fresh process observations precede primary and repeat after this module
passes all required qualification and later matrix work is explicitly deferred.
Deferred checks are not counted as passed. The process barriers require no
matched compiler/test jobs to remain active. Whole-desktop idleness, statistical significance and
architecture-independent performance are not claimed. Lower ratios mean lower
observed median latency on this host.

## Observed latency relative to the safe backend

| Campaign | Backend | Compression bound | Skippable writing |
| --- | --- | ---: | ---: |
| primary | safe | 1.0000 | 1.0000 |
| primary | unsafe-bitstream | 1.4590 | 1.5105 |
| primary | unsafe-byte-cursor | 1.1371 | 1.1275 |
| primary | unsafe-dictionary | 1.1973 | 1.3023 |
| primary | unsafe-fse | 1.3092 | 1.0988 |
| primary | unsafe-fse-encoder | 1.0733 | 1.0797 |
| primary | unsafe-history | 1.0208 | 0.9288 |
| primary | unsafe-huffman | 1.3138 | 1.3039 |
| primary | unsafe-huffman-codebook | 0.9353 | 1.0185 |
| primary | unsafe-in-place-buffer | 0.9680 | 0.9422 |
| primary | unsafe-match-chain | 1.0856 | 1.0530 |
| primary | unsafe-match-table | 1.3167 | 1.2045 |
| primary | unsafe-optimal-frequencies | 1.1602 | 1.1666 |
| primary | unsafe-optimal-matches | 1.4330 | 1.3092 |
| primary | unsafe-optimal-paths | 1.5160 | 1.4299 |
| primary | unsafe-optimal-short-key | 1.0953 | 1.1164 |
| primary | unsafe-reference-chain | 1.3138 | 1.3349 |
| primary | unsafe-reference-tree | 0.9514 | 0.9427 |
| primary | unsafe-ring-history | 1.1953 | 1.3549 |
| primary | unsafe-row-table | 1.0854 | 1.1962 |
| primary | unsafe-slice-history | 1.5105 | 1.4744 |
| primary | unsafe-training | 1.2235 | 1.1718 |
| primary | all | 1.5077 | 1.5140 |
| repeat | safe | 1.0000 | 1.0000 |
| repeat | all | 1.0099 | 1.0088 |

## Qualification and retained evidence

All eleven clock-free rows agree across every retained build. Rows include each
scalar input, real payload/capacity/variant, input/output identity and typed result.
Successful payloads/unused suffixes and rejected destinations retain their guards.
These are result/guard qualification records, not dynamic allocator counters.
Separate exact-production core-only component checks establish no allocator
dependency; the full codec remains no_std plus alloc.

The archive retains exact source/harness, generated-workload recipe, all retained
executables, qualification rows, raw samples, summaries and actual CPU barriers.
No external native implementation executes inside measured operations. Evidence:
`evidence/2026-10-27-frame-utility-benchmarks.json`.

## Clock-resolution repair

The analogous per-call scalar-clock preparation was replaced before any utility timings after the query campaign exposed clock-resolution zeros. Original qualified23+2 builds remain retained. Every new batch-clock backend and independent repeat was rebuilt, requalified and timed from fresh barriers.
