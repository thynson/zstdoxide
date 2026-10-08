# Official test migration

The reference is facebook/zstd v1.5.7, commit
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`. This module ports test behavior to the
Rust implementation; calling the official implementation alone is not a passing
test of zstdoxide. Existing independently generated interoperability fixtures
remain useful but do not count as a port of the entire upstream suite.

## Scope and evidence

Inventory all upstream test programs, scripts, assets, fuzz targets, regression
cases, and test-specific build/CI entry points. Give every source a disposition
and explicit dependencies. A partially ported source remains partial. Unsupported
streaming, dictionaries, threading, legacy formats, CLI behavior, and C-specific
APIs must not disappear behind a blanket claim of coverage. Empty tests and
unconditional-success placeholders are prohibited. An ignored test needs a real
assertion body and a precise missing prerequisite; inventory-only cases remain
unported.

The implementation has an independent compression strategy, so upstream golden
compressed bytes are decoder fixtures and source-data compatibility cases.
Byte-identical encoder output is not a format requirement. Any such adaptation
must retain the original contract and clearly record what changed.

## Test structures

Test descriptors own plain strings, expected errors, lengths, and file paths.
Embedded compressed bytes are immutable slices; dynamically generated inputs
use owned vectors. The deterministic generator owns scalar PRNG state and uses
indices for repetitions. These are test-only value records and safe collections,
not new production performance backends. No unsafe code or raw owning pointers
are needed in the test infrastructure. Corpora are bounded before execution;
large/long-running campaigns are explicitly selectable.

## Work decomposition

1. Record a complete hash-based source inventory and finer named-case mappings.
2. Import upstream golden assets and independent expected output with provenance.
3. Implement one-shot and malformed decoder assertions in Rust.
4. Preserve deterministic fuzzer generation and port supported API checks.
5. Generate and replay valid decodecorpus frames independently of the compressor.
6. Record CLI/streaming/dictionary/threading/build dependencies individually.
7. Audit dispositions, execute all runnable ports against each backend, and
   report counts that distinguish files, cases, fixtures, and blocked contracts.

`TODO.md` tracks each checkpoint. A complete inventory is not equivalent to a
complete test port, and the migration stays open while required contracts remain
unimplemented.
