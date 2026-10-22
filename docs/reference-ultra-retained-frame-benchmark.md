# Bounded retained Ultra complete-frame benchmark

The frozen baseline contains 26 complete retained Ultra cases selected before
clocks: four retained source boundaries, raw/RLE/compressed histories, every
minimum-match setting, target requests and table/search endpoints. The native
qualification and projection provenance are documented in
`reference-ultra-retained-public-qualification.md` and the fixture README.

## Clock-free qualification

All 21 configurations (safe, nineteen isolated unsafe features and all features)
pass, producing 546 case rows from 47 recorded setup/build/run commands. Every
complete frame is identical to the independently native-decoded baseline and to
the safe backend's frame. Allocation surveys agree across backends and release
all owners after output destruction. No timing samples are collected.

Ordinary and optimized Python each reject 41 negative controls and accept two
positive controls without invoking child processes. The resulting reports agree
after removing the interpreter flag and normalizing their distinct output roots.
Coherent modifications across every backend still fail the independent native
baseline check.

The benchmark example and its fixture pass formatting, ordinary/optimized
baseline checking and strict safe/all Clippy. The example was added after the
full public regression snapshot, so these separate integration and all-backend
checks provide its qualification. Production Rust is unchanged.

The five integration commands, two identical no-child projections, complete
clock-free campaigns, compiled executables, effective environments, sources and
control inputs are retained in
`evidence/2026-10-21-ultra-retained-clockfree.{json,tar.gz}`. Ordinary build
intermediates may be omitted only with individual hash records; the actual
campaign executables remain preserved.

## Complete measurements and independent repeat

Completed full regression, foreign-platform, native interoperability and
clock-free receipts are bound by exact hashes before timing. The coordinator's
observed completion record identifies the finished task-owned jobs; it does not
claim operating-system idleness or exclude unrelated desktop activity. The
controller builds all 21 executables before primary measurement, then separately
builds safe/all for the independent repeat. No other task-owned CPU jobs are
started during either campaign.

Each of 26 preselected cases has five samples lasting at least 100 ms. The primary
campaign retains 546 case rows and the repeat adds 52: 598 rows, 2990 samples and
23 actual executables. Complete frames, resolved requests and allocation ledgers
agree across all backends and across the repeat. All owner identities are
released. Ordinary and optimized timed-control suites each pass 33 checks:
31 rejected forgeries and two positive controls, with no child execution.

Ratios use median compression-plus-drop nanoseconds per source byte divided by
the same-campaign safe median. Values below one are faster. Geometric means
weight the 26 cases equally; per-case minima/maxima are ratio ranges, not
confidence bounds. Raw sample ranges and executable identities remain available
in the CSV and source-bound evidence.

| Campaign | Configuration | Geometric mean cost / safe | Per-case minimum | Per-case maximum |
| --- | --- | ---: | ---: | ---: |
| primary | `safe` | 1.0000 | 1.0000 | 1.0000 |
| primary | `unsafe-byte-cursor` | 0.9884 | 0.9154 | 1.0118 |
| primary | `unsafe-bitstream` | 0.9946 | 0.9203 | 1.1100 |
| primary | `unsafe-history` | 1.0016 | 0.9545 | 1.0750 |
| primary | `unsafe-ring-history` | 0.9838 | 0.9193 | 1.0296 |
| primary | `unsafe-fse` | 1.0021 | 0.9454 | 1.0491 |
| primary | `unsafe-fse-encoder` | 1.0230 | 0.9894 | 1.1010 |
| primary | `unsafe-huffman` | 1.0044 | 0.9772 | 1.0581 |
| primary | `unsafe-huffman-codebook` | 1.0099 | 0.9890 | 1.0454 |
| primary | `unsafe-match-table` | 1.0163 | 0.9995 | 1.0528 |
| primary | `unsafe-match-chain` | 1.0159 | 0.9328 | 1.2757 |
| primary | `unsafe-dictionary` | 0.9967 | 0.9411 | 1.0479 |
| primary | `unsafe-training` | 1.0147 | 0.9553 | 1.2883 |
| primary | `unsafe-reference-chain` | 0.9936 | 0.9151 | 1.0237 |
| primary | `unsafe-row-table` | 1.0004 | 0.9399 | 1.0294 |
| primary | `unsafe-reference-tree` | 0.9869 | 0.9282 | 1.0224 |
| primary | `unsafe-optimal-frequencies` | 0.9883 | 0.9383 | 1.0173 |
| primary | `unsafe-optimal-paths` | 0.9938 | 0.9536 | 1.0197 |
| primary | `unsafe-optimal-matches` | 1.0077 | 0.9655 | 1.0406 |
| primary | `unsafe-optimal-short-key` | 1.0037 | 0.9759 | 1.0390 |
| primary | `all` | 0.9969 | 0.9709 | 1.0222 |
| repeat | `safe` | 1.0000 | 1.0000 | 1.0000 |
| repeat | `all` | 1.0104 | 0.9505 | 1.0986 |

Across this fixed selection, peak requested allocation is 274,175–657,217
bytes and allocation attempts range from 35 to 141. The observed
requested and allocated peaks agree; all backends have identical ledgers and
zero final live allocation. These are workload observations, not global memory bounds.

The all/safe geometric mean cost ratios are 0.9969 in the primary
campaign and 1.0104 in the independent repeat. These describe only this
fixed, equally weighted selection on this host. Fixed configuration order,
thermal/compiler history, timer variation and unrelated desktop activity limit
broader conclusions. Some isolated features do not participate in the measured
path. No universal unsafe speedup is established; safe remains the default.

All executables, effective environments, source inventories, CPU barriers,
frames, samples, control inputs and CSV/JSON reports are retained in
`evidence/2026-10-22-ultra-retained-frames-timed.{json,tar.gz}`. The archive was
independently reconstructed and the original files rehashed. Archive SHA-256:
`aa49c421d481ab5dec76a846fa8b8ba819d52a619d24f46989e2d5aefa5df7b4`.
