# One-shot encode selection measurements

Thirty-three fixed pairs combine empty, raw and prepared dictionaries with
eleven inputs: empty/tiny data, content-size boundaries, raw/RLE and multiblock
input, and two retained upstream golden fixtures. The stateless baseline, the
persistent coordinator and both prefix schedules borrow the same immutable
DictionaryView, write the same caller-owned destination prepared outside
clocks, and produce exactly the stateless encoder bytes. Every attempt starts
with the intended selection and ends consumed (prefix) or retained
(persistent); guards, suffixes, complete-byte equality and unchanged dictionary
bytes are checked outside clocks.

Twenty-four primary executables (safe, all 22 isolated features and all
features) and independently rebuilt safe/all repeats each retain 675 samples:
33 pairs, four compression modes and five samples, plus 15 setup-only batch
samples. Per-attempt compression clocks include all Global scratch
allocation/release; prefix-outside excludes rearming from the compression
clock while prefix-inside includes it, so each timed call is actually selected.
The setup-only observation clocks whole 4096-operation arm/observe/clear
batches; both armed and cleared states are black-box observable so
optimization cannot remove the assignment pair. That batch includes loop and
black-box overhead and is not a directly subtractable estimate of per-attempt
overhead. Immutable dictionary construction and caller destination setup stay
outside every clock.

Each executable retains 297 identical qualification rows across stateless,
persistent and prefix configurations. The original allocator owns one-block
staging; independent clones own workspace. A separate dictionary family
records construction/release with unchanged snapshots through every selected
call; these ledger counts are not timed scratch counts. Timed calls use
Global without the role router. Two fresh process observations gate each
campaign without claiming desktop idleness.

| Campaign | Backend | Mode | Reference | All nonempty pairs | Empty dictionary | Raw dictionary | Prepared dictionary |
| --- | --- | --- | --- | ---: | ---: | ---: | ---: |
| primary | safe | persistent | safe stateless | 1.0047 | 1.0095 | 0.9863 | 1.0185 |
| primary | safe | prefix-outside | safe stateless | 1.0010 | 0.9907 | 0.9959 | 1.0166 |
| primary | safe | prefix-inside | safe stateless | 1.0299 | 1.0578 | 1.0174 | 1.0151 |
| primary | safe | prefix-inside | safe prefix-outside | 1.0289 | 1.0677 | 1.0216 | 0.9985 |
| primary | all | stateless | safe stateless | 1.0233 | 0.9593 | 1.0731 | 1.0410 |
| primary | all | persistent | safe persistent | 1.0215 | 0.9584 | 1.0916 | 1.0189 |
| primary | all | prefix-outside | safe prefix-outside | 1.0161 | 0.9636 | 1.0695 | 1.0180 |
| primary | all | prefix-inside | safe prefix-inside | 0.9936 | 0.9119 | 1.0545 | 1.0202 |
| primary | all | persistent | all stateless | 1.0029 | 1.0086 | 1.0032 | 0.9969 |
| primary | all | prefix-outside | all stateless | 0.9939 | 0.9952 | 0.9925 | 0.9942 |
| primary | all | prefix-inside | all stateless | 1.0000 | 1.0055 | 0.9998 | 0.9948 |
| primary | all | prefix-inside | all prefix-outside | 1.0061 | 1.0104 | 1.0073 | 1.0006 |
| repeat | safe | persistent | safe stateless | 1.0075 | 1.0035 | 1.0205 | 0.9987 |
| repeat | safe | prefix-outside | safe stateless | 1.0025 | 1.0016 | 1.0135 | 0.9925 |
| repeat | safe | prefix-inside | safe stateless | 1.0004 | 1.0110 | 1.0128 | 0.9778 |
| repeat | safe | prefix-inside | safe prefix-outside | 0.9979 | 1.0094 | 0.9993 | 0.9852 |
| repeat | all | stateless | safe stateless | 1.0049 | 0.9746 | 1.0335 | 1.0075 |
| repeat | all | persistent | safe persistent | 0.9993 | 0.9789 | 1.0172 | 1.0022 |
| repeat | all | prefix-outside | safe prefix-outside | 1.0028 | 0.9751 | 1.0288 | 1.0052 |
| repeat | all | prefix-inside | safe prefix-inside | 1.0094 | 0.9775 | 1.0319 | 1.0198 |
| repeat | all | persistent | all stateless | 1.0019 | 1.0080 | 1.0044 | 0.9935 |
| repeat | all | prefix-outside | all stateless | 1.0004 | 1.0021 | 1.0089 | 0.9902 |
| repeat | all | prefix-inside | all stateless | 1.0049 | 1.0141 | 1.0112 | 0.9897 |
| repeat | all | prefix-inside | all prefix-outside | 1.0045 | 1.0119 | 1.0023 | 0.9994 |

Ratios are equal-weight geometric means of per-pair median latency ratios,
using 30 nonempty pairs overall and ten per dictionary kind. The three
empty-input pairs remain in raw data. Values below one mean lower observed
numerator latency on this host. Fixed order and five samples do not establish
statistical significance, a universal unsafe benefit or elimination of scratch
allocation. The coordinator claim is a borrow on the persistent path and a
small ownership move on the prefix path; ratios near one are consistent with
that machinery, not proof of zero cost. Setup-only batch medians are retained
separately in the evidence and are not subtracted from any compression clock.
All executables, source, fixture payloads, raw samples and barriers are
retained. Evidence: `evidence/2026-10-29-one-shot-encode-benchmarks.json`.
