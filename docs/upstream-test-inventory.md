# Official test migration inventory

## Design and decomposition

The reference is facebook/zstd v1.5.7, revision
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`. The migration tracks upstream
test intent separately from whether the current Rust API can execute it.
Copying a fixture or listing a source file is not a successful test port.

The inventory uses three plain value records:

- A source record owns a relative path, SHA-256 digest, Git object identifier,
  role, suite, migration status, explanation, and local test references.
- A case record owns an identifier, source line range, original test label,
  migration status, explanation, and local test references. Ranges are integer
  indices into the referenced source; there are no non-owning pointers.
- A scope record owns a path and exclusion reason for each upstream file that
  is not a test, test dependency, test asset, or test entrypoint. Together with
  source records these classify the complete pinned Git tree.

The JSON manifest owns its strings and lists. The audit uses Python's managed
values and Rust test adapters use ordinary borrowed slices and owned vectors.
These are metadata records, not performance-sensitive codec structures, so
an artificial unsafe backend would add no useful comparison. Upstream
counterparts are Make/CMake/Meson/CI target definitions, C test labels, Google
Test declarations, shell scripts, and golden-file directories.

The work is decomposed into: catalog every upstream source and test entrypoint;
extract stable source case anchors; verify hashes, scope, references, and
status invariants; map executable Rust adaptations after focused tests pass;
document the remaining API dependencies. The coordinating checklist is in
`TODO.md` and codec ownership analysis is in `docs/data-structures.md`.

## Status semantics

- `ported`: the complete named unit has a runnable Rust test adaptation and
  local references; there is no omitted assertion or unsupported parameter
  within the declared unit.
- `partial`: a useful part runs, but the explanation identifies what remains.
- `planned`: identified work for an available API that has not been completed.
- `blocked`: test intent requires an API or format feature not implemented.
- `non_applicable`: documentation, support/build metadata, or a test specific
  to a C/C++ implementation detail without a Rust counterpart. The reason must
  be explicit. Unsupported codec features are never classified this way.

Case anchors identify named upstream test groups, not individual executions of
every seed, parameter combination, or assertion in a loop. A whole-file source
record remains authoritative for unlabelled assertions and randomized drivers.
Case counts must not be presented as full upstream assertion counts.

## Complete source scope

`tests/upstream/manifest.json` classifies all **639** tracked files in the
pinned tree: **418** test-related files and **221** explicit non-test
exclusions. All **222** files under upstream `tests/` are included, including
dotfiles, expected CLI output, symlinks, support code, and binary fixtures.
The larger catalog also includes the test paths outside `tests/`:

| Suite | Files | Named case groups |
| --- | ---: | ---: |
| Core, streaming, stress, and shell integration | 32 | 513 |
| CLI scripts, assertions, and support | 102 | 0 |
| libFuzzer targets and support | 37 | 28 |
| Golden corpora | 14 | 0 |
| Compression regression framework | 14 | 0 |
| Gzip compatibility | 23 | 0 |
| pzstd and C++ utility tests | 15 | 41 |
| Seekable format and examples | 10 | 5 |
| Kernel and freestanding integration | 19 | 6 |
| zlib wrapper and examples | 18 | 21 |
| Public API examples | 14 | 0 |
| Educational decoder and harness | 6 | 0 |
| Single-file C build checks | 18 | 0 |
| Input generation helpers | 5 | 0 |
| Build systems, CI, and other entrypoints | 91 | 1 |

The **615** named groups include 232 anchors in `tests/fuzzer.c`, 89 in
`tests/zstreamtest.c`, and 173 in `tests/playTests.sh`. A named function and a
diagnostic inside it are separate navigation anchors, so these figures are
not additive counts of independent executable tests. Shell tests without a
named heading remain individual whole-file source units. Build entrypoints
include Make, CMake, Meson, Visual Studio, GitHub Actions, and Cirrus.

The exclusions are production implementation, packaging, policy, and format
documentation. They do not hide unsupported streaming, dictionaries, legacy,
seekable, threading, CLI, or gzip test suites. Those are included and retain
their explicit unsupported status until the corresponding behavior exists.
No reference C implementation is vendored into this repository.

Upstream sources and labels retain their original names in this metadata.
The upstream root source notice attributes Meta Platforms, Inc. and affiliates
and offers a BSD-3-Clause option. Individual third-party notices still govern
their files, including the GNU gzip tests and zlib examples; this catalog does
not relicense them. Source links can be formed
as `https://github.com/facebook/zstd/blob/<revision>/<path>#L<line_start>`;
each source record includes its SHA-256 and pinned Git blob identifier.
Hashes use canonical Git bytes; the pinned `.gitattributes` CRLF conversion for
Windows project files is normalized before comparison.

## Executable mappings and remaining work

The manifest's `rust_tests` references point to actual local test declarations,
including declarations generated by test macros. The associated status and
explanation apply to the entire source unit or named group, not just the
existence of a local file.

| Upstream intent | Rust execution | Scope |
| --- | --- | --- |
| Successful and malformed golden frames | `tests/upstream_golden.rs` | Exact original inputs and output/error assertions |
| Golden compression corpus | `tests/upstream_golden.rs` | One-shot roundtrip and independent interoperability; specialized encoder settings remain missing |
| Named fuzzer cases and randomized driver | `tests/upstream_fuzzer.rs` | Exact RNG/noise workload and supported one-shot cases; full label mapping in `tests/upstream/FUZZER.md` |
| Generated complete frames and standalone headers | `tests/upstream_decodecorpus.rs`, `tests/upstream_streaming.rs` | Official generated inputs, original bytes, bounded fresh-generator replay and fragmented decoding; the generator's dictionary/raw-block branches remain unported |
| Inline legacy and skippable regression bytes | `tests/upstream_inline_regressions.rs` | Modern component runs; the full legacy desired-success assertion remains blocked |
| Four public one-shot fuzz targets | `tests/upstream_fuzz_targets.rs` | Exact tail-selected input contracts with bounded deterministic inputs and a configurable external file runner; contexts/parameters/in-place and production scanner APIs remain missing |
| Regression directory corpora | `tests/upstream_regression_corpora.rs` | Exact Silesia/GitHub inputs and full size/content roundtrips; the original method, strategy, level, dictionary, and CSV comparison matrix remains missing |
| FSE normalized-count and Huffman decoding fuzz intent | `tests/upstream/entropy.rs`, `src/entropy_encode.rs` | Official vectors, arbitrary bounded decoding and checked normalized-count writing; the original randomized writer/reader target driver and specialized C controls remain incomplete |
| Single-file DXT1 and kernel empty-frame fixtures | `tests/upstream_testcard.rs` | Exact original bytes and output contracts; maximum-level selection, browser rendering, and kernel workspace APIs remain missing |
| Ordinary streaming and context reuse | `tests/upstream_streaming.rs`, `tests/upstream_window.rs` | Bounded fragmented progress, flush/finish/reset, pledge and checksum failures, official window-distance fixtures and matching external adapters; specialized parameter/control paths remain separate |
| Prepared dictionaries and prefixes | `src/dictionary.rs`, `tests/upstream_dictionary.rs`, `tests/upstream_dictionary_encode.rs` | Exact malformed/prepared metadata, raw/formatted matches, first-block decoding entropy/repeat offsets, IDs, persistent selection, one-use prefixes and resource/state boundaries; original level/strategy and broader random schedules remain incomplete |
| Raw and formatted dictionary training | `tests/upstream_training.rs`, `tests/upstream_formatted_training.rs` | Bounded deterministic content selection, generated sample-derived literal metadata, exact cyclic input construction, held-out improvement and bidirectional C interoperability; COVER/fastCover, shrink, worker optimization and strategy-specific parameter schedules remain incomplete |
| Dictionary fuzz targets | `tests/upstream_dictionary_fuzz.rs`, `tests/upstream_trained_dictionary_fuzz.rs` | All five pinned target corpora replayed through supported APIs with exact source/sample selectors; original fastCover, by-reference lifetimes, advanced parameters and external sequence producers remain partial |

The dictionary decoder checkpoint runs **40 integration tests** and **11 parser
unit tests**. The mapped dictionary encoder checkpoints add **24 regular
tests** and two explicitly exercised CLI tests: 96 streamed and 20 one-shot
frames were independently decoded byte for byte. Raw training adds four regular
tests and a bidirectional 16-frame official-C check on held-out inputs; this is
an independent raw selection strategy. Formatted training adds seven regular
tests and a separately exercised C check accepting eight Rust-built dictionaries
and 36 source/dictionary pairs in both directions. Its cyclic assertion preserves
the exact original 1 MiB sample layout. A second independent C-backed campaign
trains on the original CNBuffer sample layout and verifies all five complete
5 MiB sources. Neither trainer implements COVER/fastCover optimization. The fixtures include the original HTTP/fixed-training/invalid
dictionaries plus **48** reproducible dictionary-related files backed by **123**
independent C oracle outcomes. C acceptance and the selected format lifetime rule
remain separate: dictionary references at P==W are accepted and references at
P>W rejected. Source-backed mappings and remaining qualifications are detailed in
`docs/upstream-dictionary-plan.md`; fixture presence alone never marks a source
test complete.

Rust's dictionary reset contract deliberately preserves selected dictionaries
through `reset` and `reset_with_options`. Upstream groups which also require C
`session_and_parameters` or `initDStream` to clear a selection therefore remain
partial; the explicit Rust operation is `set_dictionary(None)`. Similarly,
first-block entropy witnesses do not complete upstream's exhaustive code-value
or compressor-specific table-reuse sweeps.

An ignored test may contain a complete desired-success assertion while still
being `blocked` because the implementation cannot pass it. In particular,
rejecting an unsupported dictionary or legacy frame never substitutes for an
upstream test that expects successful decoding. The upstream compression
fuzzer that uses random insufficient output buffers also remains blocked:
the current allocating encoder has no equivalent caller-buffer API.

Remaining `planned` examples are ordinary file-based sample-program ports.
Remaining `blocked` suites need advanced streaming/context behavior, dictionary
encoding/training test adapters, compression parameters, legacy, raw-block,
seekable, threaded, CLI, or external-format interfaces. Their
paths and reasons remain in the machine-readable inventory so future modules
can enable actual assertions without losing the upstream correspondence.

## External corpus snapshots

`tests/upstream/external-corpora.json` separately pins fifteen rolling release
archives: twelve fuzz target corpora and three regression inputs. They are
external release snapshots observed on 2026-10-08 and 2026-10-09, not files proven to belong
to the v1.5.7 tag. Archives and extracted inputs remain in an external cache.
Their original source notices are independent of the codec's root license.

The following complete selected fuzz corpora passed the Rust adapters in
release builds with all unsafe features enabled and no skipped files:

| Corpus | Inputs | Tested contract |
| --- | ---: | --- |
| `simple_decompress` | 11,631 | Tail-selected compressed input, bounded decode, and successful content-size agreement |
| `zstd_frame_info` | 2,395 | Public frame-header parsing and metadata bounds |
| `decompress_dstSize_tooSmall` | 1,173 | Empty input success and nonempty insufficient output failure |
| `huf_decompress` | 605 | Both BMI-dependent parameter-consumption layouts; 604 successful table parses across both layouts |
| `stream_decompress` | 12,462 | Tail-selected chunks, bounded streaming progress, dictionary-free output/error parity and complete input replay |
| `stream_round_trip` | 24,223 | Complete selected source bytes, randomized supported chunk/directive schedules and exact decoded content |
| `dictionary_loader` | 2,271 | Exact tail-selected load modes; 1,919 exact roundtrips and 352 formatted-dictionary parse rejections |
| `raw_dictionary_round_trip` | 13,876 | Exact source/dictionary regions, prefix selection, checksum disabling, compression-bound destination and exact decoded content |
| `dictionary_decompress` | 12,063 | 11,768 trained dictionaries, 295 expected empty fallbacks; 20 bounded successful decodes and 12,043 malformed-source rejections |
| `dictionary_round_trip` | 30,925 | 30,722 trained dictionaries, 203 expected empty fallbacks; every source roundtrips exactly |
| `dictionary_stream_round_trip` | 24,223 | 23,821 trained dictionaries, 402 expected empty fallbacks; every adapted directive schedule restores exact content |

These are partial adapters for the original C targets because their missing
API/control paths remain absent. The 79-input FSE corpus is downloaded and
pinned; the upstream writer-based target has not been replayed. The source
inventory keeps those distinctions even when all inputs in a selected
supported adapter pass. Both streaming corpora and all five dictionary corpora also passed safe release
builds; their random C compression parameters, stable-buffer, in-place,
by-reference and sequence-producer branches are not claimed by the adapters.
The three trained-target adapters preserve all 100 original sample-range
selectors, verified against C, and use independent Rust training with the same
explicit training-failure fallback. Their fastCover path is still partial.
The dictionary streaming archive is byte-identical to the ordinary streaming
roundtrip snapshot and has now also passed its separate dictionary target.

The fifteen selected archives contain 2,682,880,516 unpacked bytes. The importer
allows a 4 GiB combined snapshot while retaining its 1 GiB per-archive ceiling.
Each extraction is further bounded by that archive's pinned unpacked size, and
all member paths, sizes, CRCs, archive hashes and extracted-cache bytes are
verified before publication. Archives remain outside the repository.

The selected regression-directory adapter also passed all **512** Silesia and
GitHub inputs (**12,407,963 bytes**) in a release build with all unsafe features.
The separately pinned GitHub dictionary is not consumed as a dictionary. This
run preserves complete input/output comparisons, while C compression method,
level, context, streaming, CLI, dictionary, and historical CSV baseline
comparisons remain unimplemented.

## Reproducible audit

Run the local structural audit with only the Python standard library:

```sh
python3 scripts/audit_upstream_tests.py
python3 scripts/check_upstream_fixtures.py
```

To additionally verify every source byte, the complete Git tree, and the
named source anchors, point it at an external v1.5.7 checkout:

```sh
ZSTD_UPSTREAM=/path/to/zstd-v1.5.7 python3 scripts/audit_upstream_tests.py
```

The audit validates statuses, source spans, duplicate IDs, local test files
and referenced Rust function names. It rejects a `ported` source with
unfinished child groups. It does not execute Rust tests; passing this audit
alone establishes inventory integrity, not behavioral parity.

The separate fixture audit checks all bytes listed by the golden,
decodecorpus, entropy, inline-regression, testcard, window-sequence, and
dictionary manifests.
It verifies sizes, SHA-256 values, upstream source hashes, local oracle source
hashes, derived byte ranges, and recorded expected-output metadata. Missing
fixtures/manifests, unlisted files, duplicate records, and paths outside a
fixture collection are errors. It uses only Python's standard library and
does not require a C compiler, upstream checkout, codec executable, or network.
Hash integrity does not replace the Rust assertions that actually decode the
fixtures; oracle regeneration remains available in each fixture importer's
`--check` mode.

The completion gate is deliberately stricter:

```sh
python3 scripts/audit_upstream_tests.py --require-complete
```

This command must fail while any source test unit is partial, planned, or
blocked. It must also fail while external target coverage remains partial or
blocked, even after selected archives have been separately pinned and their
supported adapters pass. Downloaded archives are not in the v1.5.7 Git tree;
independent snapshot pins do not establish tag membership. An ordinary
`cargo test` cannot imply that optional external corpora were run.
