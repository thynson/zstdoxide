# Dictionary-backed caller-owned compression measurements

Thirty-three fixed pairs combine empty, raw and prepared dictionaries with eleven
inputs: empty/tiny data, content-size boundaries, raw/RLE and multiblock input,
and two retained upstream golden fixtures. Every output exactly matches the
existing Rust allocating dictionary encoder. Caller guards and unused success
suffixes are checked outside timing; C byte identity is not a contract.

Twenty-four primary executables (safe, all 22 isolated features and all features)
and independently rebuilt safe/all repeats each retain 330 samples: 33 pairs,
two operations and five samples. Per-operation clocks include all Global scratch
allocation/release. Allocated mode includes returned destination allocation and
destruction; borrowed mode uses a preallocated caller buffer. Immutable dictionary
construction, input/buffer setup, byte/guard verification and accounting are
outside both clocks. Both operations borrow the same DictionaryView. Two fresh
process observations gate each campaign without claiming desktop idleness.

Each executable retains 198 identical qualification rows. The original allocator
owns destination or one-block staging; independent clones own workspace. A separate
dictionary family records construction/release, with exact unchanged snapshots
through compression. These dictionary ledger counts are not timed scratch counts.
Every owner releases through its own domain. Borrowed staging stays within one
raw block plus its header. Timed calls use Global without the role router.

| Campaign | Backend | Operation | Reference | All nonempty pairs | Empty dictionary | Raw dictionary | Prepared dictionary |
| --- | --- | --- | --- | ---: | ---: | ---: | ---: |
| primary | safe | borrowed | safe allocated | 0.9997 | 1.0000 | 0.9982 | 1.0008 |
| primary | all | allocated | safe allocated | 0.9976 | 1.0082 | 0.9802 | 1.0046 |
| primary | all | borrowed | safe borrowed | 0.9970 | 1.0300 | 0.9569 | 1.0055 |
| primary | all | borrowed | all allocated | 0.9990 | 1.0217 | 0.9744 | 1.0016 |
| repeat | safe | borrowed | safe allocated | 0.9918 | 0.9899 | 0.9957 | 0.9899 |
| repeat | all | allocated | safe allocated | 1.1435 | 1.2085 | 1.1479 | 1.0779 |
| repeat | all | borrowed | safe borrowed | 1.1365 | 1.2231 | 1.1291 | 1.0630 |
| repeat | all | borrowed | all allocated | 0.9857 | 1.0018 | 0.9794 | 0.9762 |

Ratios are equal-weight geometric means of per-pair median latency ratios, using
30 nonempty pairs overall and ten per dictionary kind. The three empty-input
pairs remain in raw data. Values below one mean lower observed numerator latency
on this host. Fixed order and five samples do not establish statistical
significance, a universal unsafe benefit or elimination of scratch allocation.
All executables, source, fixture payloads, raw samples and barriers are retained.
Evidence: `evidence/2026-10-27-dictionary-slice-encode-benchmarks.json`.
