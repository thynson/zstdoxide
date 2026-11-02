# Parameterized dictionary one-shot measurements

Forty-eight fixed case/level pairs combine three immutable dictionary kinds
(empty, raw and prepared) with seven inputs: empty and tiny data, 1.5 KiB text,
8 KiB deterministic noise, a 65,792-byte block boundary, a 131,073-byte RLE
block and a 393,233-byte multi-block source. Levels 1-6 and 9 cover all six
admitted combined-staging profiles; the largest input omits only level 9,
whose hash log exceeds the documented Lazy2 cap. Each pair resolves the same
request against the same source and borrowed `DictionaryView`, writes the same
caller destination prepared outside clocks and produces exactly the stateless
bytes in the persistent coordinator.

Twenty-four primary executables (safe, all 22 isolated features and all
features) and independently rebuilt safe/all repeats each retain 1455 samples:
144 pairs, two configurations and five samples, plus 15 setup-only batch
samples. Per-attempt compression clocks include all Global scratch
allocation/release. Dictionary construction, caller destination setup,
complete-byte/guard validation and allocation ledgers stay outside clocks. A
separate tracked dictionary domain must remain unchanged through every call
while the tracked scratch domain must be fully released; stateless and
coordinator configurations record identical 576-row ledgers per executable.
The setup-only observation clocks whole 4096-operation coordinator
construction/observation/drop batches; that batch includes loop and black-box
overhead and is not a directly subtractable estimate of per-attempt overhead.
Two fresh process observations gate each campaign without claiming desktop
idleness.

Coordinator/stateless ratios are equal-weight geometric means of per-pair
median latency ratios over 123 nonempty pairs (41 per dictionary kind).

| Campaign | Backend | All nonempty | Empty dictionary | Raw dictionary | Prepared dictionary |
| --- | --- | ---: | ---: | ---: | ---: |
| primary | safe | 1.0025 | 1.0022 | 0.9988 | 1.0065 |
| primary | all | 0.9996 | 0.9995 | 1.0015 | 0.9979 |
| repeat | safe | 1.0038 | 1.0010 | 1.0050 | 1.0055 |
| repeat | all | 1.0013 | 0.9986 | 1.0046 | 1.0006 |

| Campaign | Backend | Fast | DoubleFast | Greedy | Lazy | Lazy2 | BinaryTreeLazy2 |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| primary | safe | 0.9954 | 1.0249 | 0.9973 | 1.0009 | 0.9925 | 0.9940 |
| primary | all | 0.9984 | 1.0052 | 0.9982 | 0.9970 | 0.9989 | 0.9974 |
| repeat | safe | 0.9978 | 1.0334 | 0.9853 | 0.9992 | 0.9958 | 1.0052 |
| repeat | all | 1.0009 | 1.0023 | 1.0055 | 1.0021 | 0.9956 | 0.9973 |

Values below one mean lower observed coordinator latency on this host. Fixed
order, five samples and one host do not establish statistical significance, a
universal profile benefit or elimination of per-call staging. The coordinator
adds only a selection claim and a dictionary-view borrow; ratios near one are
consistent with that machinery, not proof of zero cost. Setup-only batch
medians are retained separately in the evidence and are not subtracted from any
compression clock. All executables, source, raw samples and barriers are
retained. Evidence: `evidence/2026-11-02-one-shot-encode-parameters-benchmarks.json`.
