# Negative-level inherited input and original C frames

This collection freezes an **exact extracted source-state replay** of
facebook/zstd v1.5.7 `tests/fuzzer.c:3468-3489` at revision
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`. It does not execute or claim successful
execution of the preceding complete unit-test harness. The C oracle is test
support, never a dependency of the Rust codec.

The target receives the first 1 MiB of an inherited 5 MiB CNBuffer, with seed 0,
compressibility 50%, automatic literal probability, and the original
all-compressors gate. Four unchanged writer stages are extracted from the pinned
source: full regeneration at 1541, the first 30 bytes at 1640-1642, the first 31 KiB
regeneration at 2556, and byte 0 = 1 at 3447. The independent source audit found no
other output alias after 1541. The immutable `extraction.json` stores original
line spans, verbatim statements and their hashes. The complete reference C
sources remain outside this repository.

`source.raw` is exactly 1,048,576 bytes, SHA256
`f20362d4a47fd3a1f1b0be21d8d7828bd43ee6ba7cdadafc697b3b0589dcea2a`.
The C helper preserves the original 5,263,360-byte destination and fresh advanced
CCtx, with only the compression-level setter set to -1 and no dictionary.
`simple.zst` and `advanced.zst` each contain 566,817 bytes with SHA256
`60d4310d442ea7e0e1e7e929665efe3d34364b0cb6494ba201201c26b38dd0ef`.

The original assertion checks non-error and equal compressed sizes. Complete
compressed-byte equality was additionally observed. Both frames have eight
compressed blocks, window 524,288, content size 1,048,576, dictionary ID 0 and no
checksum. Query and observed advanced parameters are
`[window19,chain12,hash13,search1,minMatch6,target1,Fast]`. Simple API applied
private-context fields are recorded as unavailable, not inferred observations.

Normal and scoped ASan/UBSan runs produced identical source states, records and
frames. Every frame was independently C-decoded and compared with the full
1 MiB source. The unchanged existing safe Rust `support::noise` implementation
independently replayed and compared every byte of all four 5 MiB stages plus the
final 1 MiB prefix: 22,020,096 compared bytes per mode, 44,040,192 total. No Rust Fast
or codec implementation was linked into that source comparison. Full 5 MiB stage
files remain in the external capture directory; hashes and comparison evidence
are preserved, and the checked-in input is only the final 1 MiB prefix.

ASan/UBSan instrumented this helper and unchanged datagen.c, zstd_compress.c,
zstd_fast.c and their inline helpers. Other code in the pinned static archive
was not instrumented; LeakSanitizer was disabled. This is not full-codec
sanitizer coverage. The initial compile-only failure from redundant KB/MB
macros is preserved in the evidence; no input or assertion was changed to
obtain a pass.

Validate without C or subprocesses:

```sh
python3 scripts/import_negative_level_fixture.py --check
python3 -O scripts/import_negative_level_fixture.py --check
```

Explicit regeneration uses an external clean pinned checkout with lib/libzstd.a,
an available C compiler and the pinned Rust toolchain. Choose a fresh directory
outside the repository, reference checkout and fixture collection:

```sh
python3 scripts/import_negative_level_fixture.py \
  --upstream /tmp/zstdoxize-upstream-1.5.7 \
  --output /tmp/zstdoxide-negative-level-reproduction
```

The wrapper verifies the frozen driver before importing it, protects output and
report paths, checks every source identity, executes normal/sanitized C captures
and the independent Rust source comparison, then compares complete fixture bytes
and logical observations with this capture. It retains every command/log and
failed diagnostic. `--check` never starts a compiler or any subprocess.

These are C/reference fixture facts only. They do not claim Rust numeric API
coverage, live CCtx parity, complete upstream test migration, or Rust compressed
bytes identical to C. Public Rust assertions must be validated separately.

See the [Fast execution design](../../../docs/reference-fast-design.md) and
[oracle design](../../../docs/reference-fast-oracle-design.md). The consolidated
evidence is intended for `docs/evidence/2026-10-09-negative-level-fixture.json`.
