# Bounded Fast sequence-collector observations

This collection preserves 46 bounded cases for unchanged
`ZSTD_generateSequences` from facebook/zstd v1.5.7, revision
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`, each with two initialized padding
patterns. Forty source recipes are fixed before C execution. Six capacity
variants reuse the same source bytes and derive only the caller capacity from
an observed successful row count. All recipes, envelopes and output hashes are
bound by `manifest.json`.

Successful nonempty results are compared with separately initialized unchanged
`ZSTD_compressBlock_fast` calls using the collector's actual applied parameters,
effective block schedule and persistent candidate state. Full public/direct byte
replay, expanded long lengths, offBase intent, parser/full semantic repeat triples,
and complete canonical table changes are checked independently. Collector
candidate publication is distinct from ordinary compressed/raw/RLE publication.

Per padding and build, 33 nonempty cases succeed, one empty case returns no rows,
eight valid-source cases return error106 (`sequenceProducer_failed`), and four
insufficient-capacity cases return error70 (`dstSize_tooSmall`). The successes
contain 44 candidate blocks / 69 public rows. Error and empty paths never inspect
private context/table state; partial output on error is not presented as success.
The final capture ran both padding patterns under normal and scoped ASan/UBSan
builds: 184 C case executions with exact observation equality.

The long-literal witness records LL65536 with an explicit distance65536. It does
not expose the collector's known use of raw U16 literal length during repeat
updates; that remains an unobserved limitation. Fast's third working repeat cell
is recorded separately from the full three-cell semantic replay. The maximum
block parameter131072 is also separate from observed effective block size1024
under window10.

The three original helper/driver/control files are byte-identical to the frozen
capture. They are test oracles only. Cargo does not compile or bind their C code.
The two Python support files are counted among the 50 hashed collection files;
the C helper has its separate generator hash.

Run all identity, recipe, source-catalog and semantic checks without a compiler,
reference checkout, network access or subprocess:

```sh
python3 scripts/import_fast_collector.py --check
python3 -O scripts/import_fast_collector.py --check
```

Explicit regeneration requires a clean pinned reference checkout with its
`lib/libzstd.a` already built. Output must be a fresh directory outside the
repository, fixture collection and reference checkout. An optional report must
also be disjoint from those locations and the output directory.

```sh
python3 scripts/import_fast_collector.py \
  --upstream /path/to/zstd-v1.5.7 \
  --output /tmp/zstdoxide-fast-collector-regenerated \
  --report /tmp/zstdoxide-fast-collector-report.json \
  --sanitize
```

Regeneration compares all public and direct corroboration records exactly with
the frozen files; native host layout metadata is validated separately. Sanitizer
instrumentation covers the helper plus unchanged `zstd_compress.c`, `zstd_fast.c`
and their inline headers. Remaining archive support is uninstrumented and
LeakSanitizer is disabled. The saved capture was native arm64 macOS execution;
it does not establish cross-platform execution.

The preserved negative-control runner can be used against a completed external
normal/sanitized capture. It checks 20 malformed packets per build, 32 malformed
records per Python mode, and five protected-path overlaps per Python mode:
118 checks, including four positive controls. Both ordinary Python and `-O` are
covered. It does not modify the collection.

```sh
python3 tests/upstream/fast-collector/negative_controls.py \
  --capture /tmp/zstdoxide-fast-collector-regenerated \
  --output /tmp/zstdoxide-fast-collector-negative-controls \
  --upstream /path/to/zstd-v1.5.7 \
  --source-manifest tests/upstream/fast-collector/manifest.json
```

See [the oracle design](../../../docs/reference-fast-oracle-design.md) and
[the capture evidence](../../../docs/evidence/2026-10-09-fast-collector.json).
This collection does not complete the upstream `generate_sequences` fuzz target:
dictionaries, other strategies, arbitrary parameters/capacities, sequence
recompression and merged delimiters remain separate work. No Rust Fast execution
or ordinary-frame publication result is inferred from collector success.
