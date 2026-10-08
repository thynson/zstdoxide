# Upstream test migration validation

Reference source: facebook/zstd v1.5.7, revision
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`. Validation was performed on
2026-10-08 on macOS/aarch64 with the pinned Rust 1.89.0 toolchain. The official
C executables supply fixture generation and independent answers only; the
production Rust library has no codec dependency or C fallback.

This migration is **incomplete**. The source inventory covers the entire pinned
tree, but streaming, dictionaries, contexts, CLI behavior, legacy decoding,
encoder parameters, threading, and other missing interfaces prevent full parity.
The strict completion gate must continue to fail until these contracts have
real passing Rust assertions. No percentage derived from files, diagnostic
labels, fixture counts, or random seeds represents overall compatibility.

## Runnable offline coverage

Each normal default-feature build runs **140 passing tests**: 57 unit tests,
82 integration tests, and one documentation example. Nine tests are ignored by
default: eight explicit external-oracle/corpus or longer campaign entry points,
and the known failing full-legacy contract. The eight runnable opt-in entry
points were also executed with their prerequisites; the public corpus runner
was invoked separately for each of its three downloaded target corpora.

| Rust suite | Main upstream behavior exercised |
| --- | --- |
| `upstream_golden` | All original golden data, exact successful decode/error cases, adapted compression inputs and explicit dictionary dependency |
| `upstream_fuzzer` | Original 5-MiB baseline, exact RNG/data generator, sequence/literal growth schedules, zeros, checksums, truncation, concatenation, skips, and 256 deterministic cycles |
| `upstream_decodecorpus` | 96 independently generated complete frames and 16 standalone headers, metadata, bounds, truncation, checksums and concatenation |
| Nested `upstream_entropy_tests` | 512 exact normalized-count vectors, 30 upstream Huffman tables with one/four streams, reuse, and 4,096 bounded arbitrary inputs |
| `upstream_inline_regressions` | Original current-format legacy component and exact skippable regressions; older legacy success remains ignored and failing |
| `upstream_fuzz_targets` | Four public-API target adaptations with 347 deterministic inputs each and exact C-verified tail parameter selection |
| `upstream_testcard` | Exact 32-KiB DXT1 texture and 13-byte kernel empty frame, roundtrip, checksum and boundary checks |
| `upstream_regression_corpora` | Bounded external-file planning and per-file one-shot byte equality; parameter/ratio baselines remain unsupported |

The fixture audit verifies **243 binary/source-data files in five collections**,
including exact SHA-256, length, source provenance, and expected-output metadata.
Generated Rust tables, Markdown, manifests, and notices are not counted as
binary fixture files. FSE and Huffman binary files contain multiple vectors.
Golden import includes all 12 original upstream assets plus five independently
generated compatibility frames. The inline importer retains dictionary and
old-format assets even where their consuming APIs do not yet exist.

## Explicit external runs

| Campaign | Completed workload | Result |
| --- | ---: | --- |
| One-shot fuzzer stress | 30,000 cycles, seed 305419896, upstream maximum sample log 22, five 8-MiB profile buffers | Passed with release/all-feature build |
| Official decodecorpus generation | 4,096 additional complete frames, initial seed 1 | Every decoded byte matched the independent original buffer |
| Rolling `simple_decompress` corpus | 11,631 files | All passed through the supported target adaptation; no files skipped |
| Rolling `zstd_frame_info` corpus | 2,395 files | All passed through the header-parser adaptation; no files skipped |
| Rolling `decompress_dstSize_tooSmall` corpus | 1,173 files | All passed through the explicit output-limit adaptation; no files skipped |
| Rolling `huf_decompress` corpus | 605 files, both CPU-flag parameter-consumption paths | Passed; 604 paths produced valid tables |
| Regression-data Silesia/GitHub | 12 + 500 files; 12,407,963 total source bytes | Exact one-shot roundtrip passed for every file |

External corpus runs use release builds with all unsafe backends enabled.
The four fuzz-corpus runs cover **15,804 files**, not 15,804 distinct test
contracts. The FSE fuzz corpus's 79 inputs and the prepared GitHub dictionary
were downloaded and pinned but are **not** counted as replayed: their writer
and dictionary-use contracts still require implementation. Sixteen other
official fuzz corpus archives were not downloaded or replayed.

The external archives are independently SHA-256 pinned snapshots observed on
2026-10-08. Their release assets are maintained separately from the v1.5.7 Git
tag. The eight cached archives total 39,182,154 bytes and expand to 261,416,343
bytes. Upstream's three regression-archive XXH64 pins were also verified.
The downloader enforces explicit expansion/path/type bounds; archives and
their extracted contents are kept outside the repository.

## Verification commands and limitations

`sh scripts/check.sh` performs inventory and fixture audits, formatting,
default tests, each of the six unsafe features alone, all features together,
release/all-feature tests, no_std compilation for every backend, and Clippy
with warnings denied. `scripts/check_upstream_oracles.sh` verifies the external
source revision, regenerates every checked-in collection, and runs official compatibility and
longer campaign tests. CI has separate offline and pinned-oracle jobs; a local
run does not imply a hosted GitHub Actions execution has already occurred.

The complete local matrix passed after the final test-harness fix: default,
six individual features, all features, and release/all-feature configurations
each ran 140 passing tests with nine deliberate ignores. All corresponding
no_std checks, both Clippy configurations, formatting, and source/fixture
audits passed. The oracle orchestration script also passed fixture regeneration,
both existing interoperability tests, golden encoder interoperability,
4,096 new official frames, and the complete 30,000-cycle stress run.

Parallel matrix execution exposed a timestamp-only temporary-directory name
collision in the test harness. Corpus helpers now add an atomic sequence and
retry exclusive creation on an existing name. Both affected suites passed with
64 test threads, followed by the successful complete matrix above.

The new normalized-frequency metadata test passed Miri with all features.
Existing targeted Miri jobs for every unsafe family remain in CI. The metadata
parser was separated from table construction so exact single-symbol frequency
headers can be tested without accepting them as compressed decoding tables.
The internal four-stream helper is crate-visible for the external entropy
target; its checks and production behavior are unchanged.

`decodes_all_five_official_legacy_frames` is a real desired-success assertion
that currently fails with `InvalidMagic`. It is explicitly ignored until
v0.4-v0.7 support exists. Never interpret an indiscriminate `--ignored` run as
the supported oracle command: select the named external runners and provide
their documented prerequisites. Other ignored tests perform real assertions
and require a corpus, an external oracle, or an explicit long-run budget.

The inventory and fixture auditors were also checked with deliberately broken
metadata and copies of corrupted/missing assets. They reject missing source
coverage/anchors, nonexistent test references, false complete status, changed
fixture bytes, omitted fixtures, and inconsistent provenance. Importers passed
their reproducibility checks; no generated corpus exposed a new codec failure.

Current missing obligations are retained in `TODO.md` and each source/case's
status in `tests/upstream/manifest.json`. The completed inventory must not be
confused with completion of the requested all-tests migration.
