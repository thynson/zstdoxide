# Caller-owned compression measurements

Eleven fixed inputs cover empty/tiny data, content-size thresholds, a full raw
block, RLE across a block boundary, repeated multiblock data and two retained
upstream golden-compression fixtures. Every backend emits exact existing Rust
frame bytes; guards and unused success suffixes are checked outside timing.

Twenty-four primary backends (safe,22 isolated unsafe features,all) and separately
rebuilt safe/all repeats each retain 110 samples:11 cases x 2 modes x 5 samples.
Per-operation clocks include all scratch allocation and release. Allocated mode
also includes returned destination allocation/destruction. Borrowed mode uses an
already allocated caller buffer. Input/buffer construction, byte verification,
tracking ledgers and result formatting are outside clocks. Timer overhead remains
in both modes. Two fresh process observations gate each campaign; this does not
prove the whole desktop is idle.

Clock-free Roles allocator routing distinguishes the original allocator domain
(destination for allocated output, staging for borrowed output) from independent
workspace clones. All 44 rows per backend agree exactly; every owner releases through
its own allocation domain. Borrowed staging peaks at no more than one raw block
plus header. Caller-owned destination storage is excluded from allocator counts.
Timed calls use Global directly; the role-router is not part of codec production.

| Campaign | Backend | Compared operation | Reference | Nonempty median-latency ratio |
|---|---|---|---|---:|
| primary | safe | borrowed | safe allocated | 0.9737 |
| primary | all | allocated | safe allocated | 0.9117 |
| primary | all | borrowed | safe borrowed | 0.8247 |
| primary | all | borrowed | all allocated | 0.8808 |
| repeat | safe | borrowed | safe allocated | 1.0338 |
| repeat | all | allocated | safe allocated | 1.0808 |
| repeat | all | borrowed | safe borrowed | 0.8987 |
| repeat | all | borrowed | all allocated | 0.8596 |

Ratios are equal-weight geometric means of ten nonempty per-case median latency
ratios. Values below 1 mean the numerator took less time in this fixed campaign;
the empty case stays separate in the archive. Fixed order, limited repetitions
and host variation do not establish statistical significance or a general unsafe
benefit. Whole-frame exact byte compatibility does not imply native C byte
identity. Evidence: `evidence/2026-10-27-slice-encode-benchmarks.json`.
