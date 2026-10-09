# Bounded direct Fast smoke observations

This collection contains **9 recipes and 18 observations** from the unchanged
no-dictionary Fast parser in facebook/zstd v1.5.7, revision
f8745da6ff1ad1e7bab384bd1f9d742439278e99. Each recipe runs with initialized
padding bytes 0x6a and 0xc3. The helper calls ZSTD_compressBlock_fast directly;
it is an external C test oracle, never a production codec dependency.

Both normal and ASan/UBSan captures passed. Their complete records are
byte-identical. The independent Python checker reconstructs the original
source bytes, literal/match output, explicit offset/repeat intent, all three
wire repeat offsets, hash-table buckets and canonical little-endian table
fingerprints. Complete padding-pair records match except the padding selector.

This is the first **bounded-direct-smoke** checkpoint. It does not establish
Rust Fast execution, collision-branch coverage, every key width/window/target,
dictionary modes, sequence-collector behavior, ordinary-frame raw/RLE
publication, compression ratio parity, or complete upstream test migration.
Recorded tables are final state, not chronological probe/store traces.

## Offline checks and explicit regeneration

The default CI path is entirely offline:

    python3 scripts/import_fast_parser_smoke.py --check
    python3 -O scripts/import_fast_parser_smoke.py --check

It checks pinned hashes, the fixture catalog, exact source recipes, complete
coverage and semantic replay. It invokes neither a compiler nor a subprocess.

To independently regenerate, provide a clean, already-built pinned checkout
and fresh external artifact/report paths:

    python3 scripts/import_fast_parser_smoke.py \
      --upstream /path/to/zstd-v1.5.7 \
      --output /tmp/zstdoxide-fast-regeneration \
      --report /tmp/zstdoxide-fast-regeneration.json \
      --sanitize

Omit --sanitize for an ordinary optimized capture only. --cc selects a compiler
executable. The command requires the exact reference revision and unchanged
tracked source, checks the recorded source/header hashes, and preserves all
commands, compiler information, archive hashes, stdout/stderr and source
manifests. Regeneration compares the complete case records without modifying
the checked-in fixture. Host layout fields are observed separately, allowing
a future platform to be recorded without asserting that it was tested here.
The original capture is 64-bit little-endian arm64 macOS.

The output directory and report must be fresh and outside the repository.
Neither path may equal or contain the other; this protects command logs from
being overwritten by the report. These checks run before creating directories.
--check cannot be combined with regeneration options. The --collection option
supports isolated copies for malformed-fixture checks.

## Observed recipes

Each input repeats its listed ASCII pattern for exactly prefix+block bytes.
An optional prior prefix belongs to the same initialized source allocation;
it is not a dictionary attachment. The seeded cases supply one valid hash slot
for source position zero. All cases use hash log 6, effective key width 4,
chain log 6 and search log 1.

| Recipe | Pattern; prefix/block bytes | Sequence (LL, ML, distance, offBase) | Trailing |
| --- | --- | --- | ---: |
| short-seven | abcdefg; 0/7 | none | 7 |
| short-twelve | abcdefghijkl; 0/12 | none | 12 |
| repeat-one | A; 0/64 | (2,62,1,1) | 0 |
| repeat-four | abcd; 0/64 | (4,60,4,7) | 0 |
| offset-sixteen | 0123456789ABCDEF; 0/96 | (16,80,16,19) | 0 |
| prefix-explicit | 0123456789ABCDEF; 16/64, seeded | (0,64,16,19) | 0 |
| prefix-duplicate-repeats | abcd; 16/64, seeded | (1,63,4,1) | 0 |
| maximum-acceleration | abcd; 0/64, target 131072 | none | 64 |
| long-match | A; 0/65700, window log 17 | (2,65698,1,1) | 0 |

Other target lengths are zero and window logs are 10. Initial repeat offsets
are [1,4,8], except the duplicate case uses [4,4,8]. Fast updates only the first
two parser cells: prefix-explicit returns [16,1,8], while full wire replay
produces [16,1,4]. The checker preserves and distinguishes both observations.
The long-match record actually uses the long match-length metadata flag.

The short cases call a safely prepared direct parser; they do not claim to
exercise the normal compressor's tiny-block bypass.

## Bounds and serialized state

The initialized source arena has 64 leading bytes. C base is two bytes before
logical source, preserving the upstream invalid-zero convention. The initialized
suffix is 262656 bytes: maximum target + maximum block + 512. It covers
speculative pointer formation, growing search steps, and prefetch expressions,
including the maximum-target cleanup case. It does not enlarge logical input.

Each owned array has outer 64-byte guards. Literal storage has the full
32-byte upstream wildcopy slack before its guard. Sequence capacity is
floor(block length/3)+1. The helper checks unchanged source/padding, complete
literal/match reconstruction, long-length expansion, all guards, and complete
four-byte logical bounds for every final nonzero table key.

observations.jsonl begins with one metadata record, then the nine recipes in
the table order, each with padding 106 then 195. Every record contains the full
source bytes, stored literals, sequences, parser/wire repeats, and 64 before/
after hash slots. Normalized slots use null for the C invalid index; valid
positions subtract the C base offset two. Table hashes serialize U32 values
explicitly little-endian. No native padding or pointer addresses are saved.

## Provenance and instrumentation limits

The normal original capture links the pinned existing static archive.
Sanitizer capture compiles the unchanged zstd_fast.c alongside the helper with
ASan and UBSan, instrumenting Fast and its inline memory helpers. Remaining
archive support objects are not instrumented; LeakSanitizer is disabled.
There were no compiler diagnostics, sanitizer reports, guard failures, or
failed recipe assertions in the preparation.

The first successful pair of captures preceded a metadata-only addition of
fixed parameter/window fields. The final captures each ran all 18 cases again;
all case rows remained unchanged. Thus final validation is 36 C case executions,
with 36 earlier successful executions preserved separately. Independent static
review covered source setup, pointer formation, guards, replay, and final metadata.

The portable importer independently reproduced all 18 observations in each of
its final normal and sanitized runs. Its offline validation passed 41 commands
under normal Python and 41 under Python -O: four positive controls and 78
expected rejections across both modes. These cover semantic corruption, fixture
substitution, detached metadata, and overlapping output/report paths. The offline
commands used an empty executable search path. An earlier successful portable
regeneration before the report-path correction is retained separately.

The original preparation and portable-importer validation are linked from
[the evidence record](../../../docs/evidence/2026-10-09-fast-parser-smoke.json)
and [the compact archive](../../../docs/evidence/2026-10-09-fast-parser-smoke.tar.gz).
Executable bytes are omitted from the repository archive; recorded executable
hashes remain. The original external preparation directory/archive stays intact.
See NOTICE and the repository LICENSE.zstd for upstream attribution.
