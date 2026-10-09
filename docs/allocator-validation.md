# Allocator and platform validation

Module 9 is in progress. This report separates the compiler checkpoint, custom
allocation migration, and platform execution. None of these checkpoints claims
complete upstream API or test parity. The active work items are in `TODO.md`.

## Compiler checkpoint: 2026-10-09

The implementation pin is `nightly-2026-10-08`, reporting Rust
`1.101.0-nightly`, commit `1d81eb4ad9cd207e3e638bd32b17ec4fce8412a6`, LLVM
23.1.3, on `aarch64-apple-darwin`. Installation included rustfmt, Clippy,
rust-src, and Miri. `Cargo.toml` declares a conservative `rust-version` floor
of `1.101`; the supported compiler for this work is the exact dated pin.
No stable 1.101 release is claimed as tested. An actual attempt to install the
official Rust 1.100.0 distribution returned HTTP 404 for its checksum, leaving
that compiler unverified.

The standalone [standard-API probe](evidence/2026-10-09-allocator-api-probe.rs)
compiled successfully with `#![no_std]`, `#![forbid(unsafe_code)]`, and no feature
gates. It exercises owned and borrowed standard allocators, allocator-bearing
vectors, fallible reservation, and direct return of the owning vector:

```sh
rustc --crate-type lib --crate-name allocator_api_probe --edition 2024 \
  docs/evidence/2026-10-09-allocator-api-probe.rs --out-dir /tmp/zstdoxide-allocator-api-checkpoint
```

Before any generic codec migration, the unchanged Module 8 production source
and compiled test/benchmark inputs were fingerprinted before and after seven
commands using `CARGO_TARGET_DIR=target/module9-baseline`. Both fingerprints
remained identical. Default and all-feature suites each passed 534 tests with
45 explicitly ignored tests. Formatting and actual safe/all-backend no_std
checks passed. Strict safe/all-target and all-feature/all-target Clippy each
failed on new `manual_is_multiple_of` and `chunks_exact_to_as_chunks` lints.
These failures are retained as compiler compatibility evidence, not relabeled
successful checks. No ignored external-oracle or corpus campaign was rerun in
this unchanged-source checkpoint.

The [original results](evidence/2026-10-09-allocator-compiler-baseline-results.json)
preserve all commands, exit codes, counts, durations, and log hashes. Lossless
before/after source manifests are archived alongside them. The seven commands
took 90.41 seconds in total; this includes compilation and concurrent work and
is not a codec performance measurement.

Compatibility corrections use `is_multiple_of` for the positive Huffman copy
count and `as_chunks`/`as_chunks_mut` for constant-size slices. They preserve
iteration order and explicit remainder handling. Verification of the corrected
source is recorded separately from the unchanged-source baseline.

The [corrected-source results](evidence/2026-10-09-allocator-compatibility-results.json)
passed all nine final commands in 84.84 seconds: both complete suites again
passed 534 tests with 45 ignored, focused XXH64 tests passed 7 cases, all-feature
Huffman codebook tests passed 19 cases, and formatting, both true no_std checks,
and both strict all-target Clippy checks passed. Production and test/benchmark
fingerprints stayed unchanged throughout those commands. The
[correction index](evidence/2026-10-09-allocator-compiler-corrections.json)
records the eight changed files, initial lint iterations, and final evidence.
No allocator-bearing codec owner had been introduced at this checkpoint.

The Miri filter guard now honors the repository's dated toolchain instead of a
floating `+nightly`. Its [focused check](evidence/2026-10-09-allocator-compiler-miri-guard.json)
passed one real all-feature history test and deliberately rejected an absent
selector with exit code 1. This verifies the guard and selected test only, not
the full Miri workflow. Benchmark shell syntax, guard Python syntax, and the
standalone API probe's rustfmt check also passed.

Module 8 validation and benchmark records remain measurements of their original
Rust 1.89 compiler, source snapshots, and workloads. They are not retroactively
attributed to this compiler pin. Updating the CI workflow describes future CI
execution; it does not establish that the current Linux workflow has run.

## Allocation and platform checkpoints

The owner-by-owner implementation and failure-injection results will be added
as their focused checks complete. Whole-codec custom allocation, 32-bit and
big-endian execution, and no-pointer-atomics compilation remain open. The
[upstream allocation plan](upstream-allocation-plan.md) distinguishes ordinary
allocator control from static workspaces, shrinking policies, estimators,
stable buffers, and C numeric compression levels.
