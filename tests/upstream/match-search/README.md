# Pinned match-search hash observations

This collection freezes the unsalted hash primitives from facebook/zstd
**v1.5.7**, revision `f8745da6ff1ad1e7bab384bd1f9d742439278e99`. The reference is
`lib/compress/zstd_compress_internal.h:903-961`. Derived observations retain the
upstream BSD-3-Clause attribution; see `LICENSE.zstd` at the repository root.
The C helper calls the actual private pointer wrappers. It is an external test
oracle, never a production codec dependency. Cargo does not compile or link it.
The reference source and library remain outside this repository.

This is C-only primitive evidence. It does not establish Rust execution,
parser/candidate-order equivalence, numerical compression-level support,
compression-ratio parity, or completion of Module 11 or the official test port.

## Reproduction and stable provenance

Use a clean, already-built pinned reference checkout:

```sh
python3 scripts/import_match_search_fixtures.py /path/to/zstd-v1.5.7 \
  --report /tmp/zstdoxide-hash-capture.json
python3 scripts/import_match_search_fixtures.py /path/to/zstd-v1.5.7 --check \
  --report /tmp/zstdoxide-hash-check.json
python3 scripts/import_match_search_fixtures.py /path/to/zstd-v1.5.7 --check \
  --sanitize --report /tmp/zstdoxide-hash-sanitizers.json
```

The importer requires the exact revision, rejects tracked checkout modifications,
and checks all 20 recorded headers byte-for-byte against their pinned Git objects.
Ignored external build products, including `lib/libzstd.a`, are permitted. It
compiles only `hash_oracle.c`, verifies the compiler's upstream include list,
and rejects source/archive changes during the run. `CC` can select the compiler;
on macOS an explicit SDK is discovered with `xcrun --show-sdk-path`.

`manifest.json` hashes the generator, importer, pinned headers, and every
fixture. The original `capture` object records the compiler entrypoint hash,
version/flags, host, archive hash, and optional CLI metadata. These identify the
capture environment rather than stable fixture content: `--check` compares
stable provenance and exact generated data, without rewriting any collection
file. A report path inside the collection is rejected. An optional report
records the current environment, exact commands, exit codes, durations and
stdout/stderr hashes separately. A different compiler path or rebuilt archive
does not invalidate identical observations.

`--sanitize` additionally compiles an AddressSanitizer/UndefinedBehaviorSanitizer
executable with recovery disabled and compares all of its observations with the
normal optimized executable. It needs a compiler/platform with those sanitizers.
Normal regeneration does not require sanitizers. The external preparation and
final fixture validation both performed this comparison successfully.

The recorded capture executed **64-bit little-endian arm64 macOS with 32-bit C
`int`**. The helper explicitly requires eight-bit bytes, 32-bit `U32`, and 64-bit
`U64`; the importer accepts only observed 32/64-bit `size_t`, 32-bit `int`, and
recognized byte order. A later successful report on a different host records
that host independently. No native 32-bit or big-endian execution is inferred
from the current capture or from portable-looking source code.

## Actual observed functions and readable input

Each primary observation directly calls the corresponding `ZSTD_hash4Ptr`,
`ZSTD_hash5Ptr`, `ZSTD_hash6Ptr`, `ZSTD_hash7Ptr`, or `ZSTD_hash8Ptr`. It must agree
with `ZSTD_hashPtr` and `ZSTD_hashPtrSalted(..., 0)`, and its result must fit the
selected hash log. No hash algorithm is copied into the C helper. The linked
archive supplies version checks; neither a compression API nor the optional CLI
supplies the hash observations.

Every C input is a fully initialized readable eight-byte array. The pinned C
wrappers for widths five through seven may physically read all eight bytes.
The unused-byte pairs establish arithmetic independence from the extra bytes;
they do **not** authorize a Rust overread. A Rust extractor must accept an exact
selected-width slice without requiring extra readable bytes.

## Schema

TSV files include a header. Numeric fields are decimal except the explicitly
hexadecimal byte strings and `first4_le`. The tuple `(width, log, pattern)` is
unique and appears in ascending nested-loop order.

| File | Records | Fields / purpose |
| --- | ---: | --- |
| `patterns.bin` | 64 | Exactly 512 bytes: 64 initialized eight-byte input records |
| `patterns.tsv` | 64 | pattern, family, bytes; human-readable companion to the binary inputs |
| `vectors.tsv` | 2,880 | width, log, pattern, bytes, bucket; five widths 4..8, nine logs 10..18, 64 patterns |
| `invariance.tsv` | 2,304 | width, log, pattern, base, changed, base_bucket, changed_bucket, full8_base, full8_changed |
| `compatibility.tsv` | 64 | pattern, first4_le, current_rust_expression_bucket, observed_c_bucket |

Each primary record's bytes must equal its pattern. Asymmetric forward/reversed
patterns must produce different log-16 buckets for every selected width.

Invariance records cover widths 4..7 and the same nine logs and 64 patterns.
`changed` preserves bytes below the selected width and XORs **every** byte at or
above it with `0xff`. Both selected-width buckets must be equal and match the
primary observation. The same pair is also hashed at width eight: all 576 pairs
per selected width (2,304 total) have different width-eight buckets in this
capture. These are concrete sensitivity controls, not a general no-collision
claim.

## Exact input construction

The importer constructs the patterns before invoking C:

- IDs 0..7 are `0000000000000000`, `ffffffffffffffff`, `0001020304050607`,
  `0706050403020100`, `0123456789abcdef`, `efcdab8967452301`,
  `55aa55aa55aa55aa`, and `512f8b0e76543210`, in that order.
  The final record starts with the existing four-byte collision witness
  `0x0e8b2f51` in little-endian order.
- IDs 8..39 contain one set bit. For each byte index 0..7, set bit offsets
  0, 1, 6, and 7 in order; all other bits are zero.
- IDs 40..63 are 24 consecutive eight-byte records from a 32-bit xorshift state
  initialized once to `0x243f6a88`. Each output byte updates the state using
  XOR-left-13, XOR-right-17, XOR-left-5, masking left shifts and the final state
  to 32 bits, then emits its high byte. Each record consumes eight updates.

All 64 records must be distinct. The generator for pseudorandom input bytes is
not a substitute implementation of the hash under observation.

## Historical four-byte compatibility guard

The `compatibility.tsv` column `current_rust_expression_bucket` means current at
the recorded pre-Module-11 revision
`0111aef8a862e36d8b4a812ee1c9a31032ee3fef`. Its two source hashes appear under
`compatibility_reference`; they are historical attribution, not dependencies
on mutable current Rust files. Later implementation changes must not rewrite
this original compatibility record merely to follow the current source.

The guard evaluates the old expression in Python: load the first four bytes as
a little-endian `u32`, multiply by `0x9e3779b1` with unsigned 32-bit wrap, then
select the high 16 bits. All 64 values must match the actual C observations at
width four/log 16. Explicit guards require zero and `0x0e8b2f51` to map to zero,
word one to map to `0x9e37`, and `u32::MAX` to map to `0x61c8`.

This compatibility arithmetic is **not** an independent second hash oracle and
is **not** Rust execution. The upstream C wrapper results remain the source of
the primary expected values.

## Preparation diagnostics

The external preparation retained two initial setup diagnostics under
`/tmp/zstdoxide-module11-hash-observations/`: `initial-missing-sdk.stderr` and
`initial-noncanonical-dependency.stderr`. They record a missing explicit macOS
SDK and an unnormalized `..` dependency path used for Git-object verification;
neither was a hash mismatch. The final importer selects the SDK explicitly and
normalizes include paths. These environment-specific logs remain outside the
fixture collection, with their hashes preserved in the validation evidence.
