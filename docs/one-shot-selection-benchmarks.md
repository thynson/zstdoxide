# One-shot selection measurements

Eleven frozen dictionary workloads cover raw distances/crossings, prepared IDs and
omitted IDs, treeless literals/four streams, repeated tables, HTTP, an empty frame,
concatenated dependent frames and a 120,000-byte actual raw overlap. Every timed
path uses the same borrowed DictionaryView; shared-owner cloning or dictionary
preparation is not artificially charged to the stateless baseline.

## Method and limits

Twenty-three primary retained executables cover safe, all and 21 isolated unsafe
features. Independent safe/all rebuilds supply a repeat. Every executable produces
495 samples: eleven cases, nine modes and five samples, with deterministic
8..1,024 iterations. Each operation has the same per-operation timer placement.
All scratch allocation/release and frame validation occur inside decode clocks.
Fixture/output allocation, verification, hashing and ledger formatting are outside.

Stateless, persistent and prefix modes each measure direct and in-place decoding.
Prefix rearming is outside decode-only clocks; suffix restoration is outside
in-place decode-only clocks. `prefix-direct-with-setup` includes rearming;
`prefix-in-place-with-setup-restore` includes both rearming and restoration.
`setup-restore-only` restores input, sets a borrowed prefix and clears it without
decoding. It is an explicit setup observation, not an exact subtractive estimate
of decode overhead. Full result/guard/selection-state assertions are outside clocks.

Two fresh process inventories precede each primary/repeat campaign. Required
qualification has passed and no matched compiler/test jobs remain active; later
matrix work is explicitly deferred and is not counted as passed. The entire
desktop is not claimed idle. Fixed-order samples do
not establish statistical significance, architecture independence or universal
unsafe benefit. Ratios below are equal-weight geometric means of per-case median
latencies across ten nonempty output cases; empty output is retained separately.

## Coordinated decoding relative to stateless decoding

Direct columns use stateless-direct; in-place columns use stateless-in-place in
the same backend. Lower ratios mean lower observed latency on this host.

| Campaign | Backend | Persistent direct | Prefix direct | Prefix + setup direct | Persistent in-place | Prefix in-place | Prefix + setup + restore in-place |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| primary | safe | 1.0952 | 1.1375 | 1.0973 | 1.0686 | 1.0137 | 1.1214 |
| primary | unsafe-bitstream | 1.0260 | 1.0715 | 1.0494 | 0.9682 | 0.9100 | 0.9472 |
| primary | unsafe-byte-cursor | 0.9863 | 0.9904 | 1.0192 | 0.9845 | 1.0024 | 1.1291 |
| primary | unsafe-dictionary | 1.0033 | 1.1023 | 1.0320 | 0.9860 | 0.9956 | 1.1185 |
| primary | unsafe-fse | 0.9808 | 0.9631 | 1.0016 | 1.0230 | 1.0082 | 1.1399 |
| primary | unsafe-fse-encoder | 1.0021 | 1.0064 | 1.0285 | 0.9990 | 1.0024 | 1.1453 |
| primary | unsafe-history | 0.9977 | 0.9930 | 1.0524 | 1.0011 | 1.0168 | 1.1391 |
| primary | unsafe-huffman | 0.9956 | 1.0058 | 1.0156 | 1.0030 | 0.9886 | 1.1275 |
| primary | unsafe-huffman-codebook | 0.9704 | 0.9628 | 0.9877 | 1.0421 | 1.0161 | 1.1469 |
| primary | unsafe-in-place-buffer | 1.0146 | 1.0211 | 1.0411 | 0.9956 | 0.9877 | 1.1848 |
| primary | unsafe-match-chain | 1.0009 | 1.0114 | 1.0280 | 0.9989 | 0.9911 | 1.1240 |
| primary | unsafe-match-table | 1.0019 | 0.9952 | 0.9995 | 0.9962 | 1.0122 | 1.1473 |
| primary | unsafe-optimal-frequencies | 0.9800 | 0.9892 | 1.0017 | 1.0075 | 1.0181 | 1.1557 |
| primary | unsafe-optimal-matches | 1.0157 | 1.0072 | 1.0426 | 1.0056 | 1.0027 | 1.1357 |
| primary | unsafe-optimal-paths | 0.9754 | 1.0006 | 1.0275 | 0.9770 | 0.9813 | 1.1080 |
| primary | unsafe-optimal-short-key | 1.0071 | 0.9943 | 1.0234 | 1.0039 | 1.0336 | 1.1718 |
| primary | unsafe-reference-chain | 0.9625 | 0.9807 | 0.9758 | 1.0070 | 0.9965 | 1.1735 |
| primary | unsafe-reference-tree | 1.0036 | 1.0036 | 1.0352 | 0.9959 | 1.0017 | 1.1433 |
| primary | unsafe-ring-history | 1.0019 | 1.0109 | 1.0245 | 1.0068 | 1.0112 | 1.1534 |
| primary | unsafe-row-table | 1.0128 | 1.0281 | 1.0376 | 0.9918 | 1.0008 | 1.1252 |
| primary | unsafe-slice-history | 0.9827 | 0.9905 | 1.0147 | 1.0154 | 1.0113 | 1.1343 |
| primary | unsafe-training | 0.9840 | 0.9946 | 1.0208 | 1.0891 | 1.0136 | 1.1351 |
| primary | all | 1.0153 | 1.0085 | 1.0420 | 0.9968 | 1.0011 | 1.1381 |
| repeat | safe | 0.9998 | 0.9943 | 0.9959 | 0.9978 | 0.9996 | 1.1434 |
| repeat | all | 1.0146 | 1.0030 | 1.0420 | 0.9906 | 0.9760 | 1.1169 |

## All-feature decoding relative to safe decoding

| Campaign | Persistent direct | Prefix direct | Persistent in-place | Prefix in-place | Prefix + setup + restore in-place |
| --- | ---: | ---: | ---: | ---: | ---: |
| primary | 0.5751 | 0.5500 | 0.6008 | 0.6361 | 0.6537 |
| repeat | 0.9742 | 0.9683 | 0.9992 | 0.9828 | 0.9831 |

## Ownership and retained evidence

Each executable has 132 identical clock-free rows: separate dictionary/scratch
ledgers for six stateless/persistent/prefix direct/in-place paths and eleven cases.
Setup leaves tracking snapshots unchanged, dictionary preparation uses its own
allocator, dictionary snapshots remain unchanged through calls, and all owners
and storage are released. Complete output, actual guards and selection state are
checked. This does not count unrelated global harness allocations or establish
retained native-context workspace reuse. Scratch is fresh per call.

The archive retains exact harness and production identities, all measured fixture
payloads, executables, raw timing samples, ownership ledgers and CPU observations.
No native codec runs inside measured calls. Evidence:
`evidence/2026-10-27-one-shot-selection-benchmarks.json`.
