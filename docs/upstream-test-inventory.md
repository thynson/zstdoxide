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
| Generated complete frames and standalone headers | `tests/upstream_decodecorpus.rs` | Official generated inputs, original bytes, and bounded fresh-generator replay; stream/dictionary/raw-block modes remain missing |
| Inline legacy and skippable regression bytes | `tests/upstream_inline_regressions.rs` | Modern component runs; the full legacy desired-success assertion remains blocked |
| FSE normalized-count and Huffman decoding fuzz intent | `tests/upstream/entropy.rs` | Independent official vectors and arbitrary bounded inputs; encoder and specialized C control paths remain missing |

An ignored test may contain a complete desired-success assertion while still
being `blocked` because the implementation cannot pass it. In particular,
rejecting an unsupported dictionary or legacy frame never substitutes for an
upstream test that expects successful decoding. The upstream compression
fuzzer that uses random insufficient output buffers also remains blocked:
the current allocating encoder has no equivalent caller-buffer API.

Remaining `planned` examples are ordinary file-based sample-program ports.
Remaining `blocked` suites need streaming, dictionary, compression-parameter,
legacy, raw-block, seekable, threaded, CLI, or external-format interfaces. Their
paths and reasons remain in the machine-readable inventory so future modules
can enable actual assertions without losing the upstream correspondence.

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
decodecorpus, entropy, inline-regression, and available testcard manifests.
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
blocked. It must also fail until upstream's external fuzz and regression
corpora have been separately pinned and exercised. Those downloaded archives
are not in the v1.5.7 Git tree and have no archive hashes pinned by that tag;
the manifest records this limitation instead of silently counting them as
migrated. An ordinary `cargo test` cannot imply those corpora were run.
