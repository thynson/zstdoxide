# Checked structural frame-query measurements

Eleven frozen cases cover known/unknown small frames, an empty-output frame,
a 129 KiB skippable payload, three pinned compressed corpus inputs, concatenated
frames, 32,769 blocks, truncation and empty input. These queries scan framing;
they do not decode entropy or validate checksum values.

## Method and limits

Twenty-three retained primary executables cover safe, all and 21 isolated unsafe
features. Independent safe/all rebuilds provide a separate repeat. Each executable
has 220 raw samples: eleven cases, four query modes and five samples, with a
deterministic 128..1,024 iterations per sample. One clock brackets the complete
batch, retaining loop/dispatch/black-box overhead. Input construction, hashing, assertions and
result formatting occur outside the measured call.

First-frame, total advertised size, total bound and in-place margin have distinct
contracts. Their absolute latencies are retained independently; a faster mode is
not claimed a substitute for another. The all/safe ratios below compare the same
operation on the same input. Equal-weight geometric means cover the ten cases
with nonempty encoded input, including the empty-output frame. Empty input remains
in raw data, outside the aggregate. Lower ratios indicate lower observed latency.

Two fresh process observations precede each primary/repeat campaign after the
recorded task CPU jobs complete. This does not claim whole-desktop idleness.
Fixed backend order and five samples do not establish statistical significance,
architecture independence or a universal unsafe speedup.

## Observed latency relative to the safe backend

| Campaign | Backend | First frame | Total size | Total bound | In-place margin |
| --- | --- | ---: | ---: | ---: | ---: |
| primary | safe | 1.0000 | 1.0000 | 1.0000 | 1.0000 |
| primary | unsafe-bitstream | 1.1679 | 1.1742 | 1.1985 | 1.1572 |
| primary | unsafe-byte-cursor | 1.0010 | 0.9914 | 0.9886 | 1.0020 |
| primary | unsafe-dictionary | 1.1885 | 1.2019 | 1.2103 | 1.1731 |
| primary | unsafe-fse | 1.0678 | 1.0539 | 1.0712 | 1.0870 |
| primary | unsafe-fse-encoder | 1.0160 | 1.0222 | 1.0266 | 1.0139 |
| primary | unsafe-history | 1.0130 | 1.0269 | 1.0375 | 1.0385 |
| primary | unsafe-huffman | 1.0127 | 1.0273 | 1.0238 | 0.9972 |
| primary | unsafe-huffman-codebook | 1.1262 | 1.1145 | 1.1216 | 1.1021 |
| primary | unsafe-in-place-buffer | 1.0966 | 1.1094 | 1.1067 | 1.0831 |
| primary | unsafe-match-chain | 1.1272 | 1.1172 | 1.1247 | 1.0962 |
| primary | unsafe-match-table | 1.0883 | 1.1026 | 1.1219 | 1.0832 |
| primary | unsafe-optimal-frequencies | 1.1555 | 1.1506 | 1.1845 | 1.2045 |
| primary | unsafe-optimal-matches | 1.1284 | 1.1022 | 1.1049 | 1.1314 |
| primary | unsafe-optimal-paths | 1.1168 | 1.1046 | 1.1003 | 1.1210 |
| primary | unsafe-optimal-short-key | 1.1746 | 1.1783 | 1.1631 | 1.1695 |
| primary | unsafe-reference-chain | 1.1042 | 1.1306 | 1.1330 | 1.1129 |
| primary | unsafe-reference-tree | 1.0096 | 1.0103 | 1.0233 | 1.0242 |
| primary | unsafe-ring-history | 1.0265 | 1.0503 | 1.0274 | 1.0160 |
| primary | unsafe-row-table | 1.1148 | 1.1066 | 1.0916 | 1.1207 |
| primary | unsafe-slice-history | 1.1709 | 1.1649 | 1.1724 | 1.1566 |
| primary | unsafe-training | 1.0220 | 1.0056 | 1.0190 | 1.0013 |
| primary | all | 0.9940 | 0.9852 | 0.9843 | 0.9763 |
| repeat | safe | 1.0000 | 1.0000 | 1.0000 | 1.0000 |
| repeat | all | 1.0263 | 1.0853 | 1.0184 | 1.0258 |

## Qualification and retained evidence

All eleven clock-free rows match across all primary and repeat builds. Each row
records deterministic input identity and all four query outcomes. These rows are
not runtime allocation counters. Separately qualified core-only builds of the
exact production query component establish that it has no allocator dependency;
the full codec remains no_std plus alloc, and benchmark fixture construction uses
ordinary host allocation outside measured calls.

The archive retains the exact harness and production snapshot, measured fixture
payloads, all retained executables, qualification rows, raw samples, summaries,
process-observer source and actual CPU barriers. No native codec runs inside the
timed operations. Unused historical binary fixtures retain inventory hashes.
Evidence: `evidence/2026-10-27-frame-query-benchmarks.json`.

## Clock-resolution repair

The initial query campaign retained two zero-duration samples from eight per-call timings of skip-large/first-frame. Its failed receipt, all raw samples and original23+2 executables remain archived. No zero was discarded or clamped; every new batch-clock backend and independent repeat was rebuilt, requalified and timed from fresh barriers.
