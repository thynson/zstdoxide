# Validation record

Local validation date: 2026-10-08. Target: `aarch64-apple-darwin`.

## Automated matrix

`sh scripts/check.sh` passed using Rust 1.89.0. The suite contains 53 unit tests,
five fixture/round-trip integration tests, and two deterministic malformed-input
tests. The two CLI oracle tests are deliberately ignored in normal runs and
were explicitly executed separately. A runnable API documentation example is
also checked by `cargo test --doc`.

The matrix covers default safe code, each of the six unsafe features separately,
all features together, and all features in release mode with overflow checks.
Every backend also compiles without the `std` feature. Formatting and Clippy
with warnings denied pass for default and all-feature builds. The default crate
forbids unsafe code at compile time.

## Independent interoperability

Twelve fixed fixtures were generated with official zstd v1.5.7. Runtime oracle
checks additionally pass 56 official-compressed cases decoded by Rust and 31
Rust-compressed cases decoded by official zstd, in default and all-feature
configurations. See `tests/fixtures/README.md` for provenance and reproduction.

XXH64 is checked against independent upstream C vectors, including all tail
paths, stripe boundaries, misaligned slices, and inputs through 1 MiB. Tables
and bitstreams have independent small examples in addition to codec round trips.

## Unsafe checks

Miri from `nightly` (rustc 1.101.0-nightly, 2026-10-07) passed targeted tests for:

- byte cursor failure and end-of-buffer behavior;
- reverse-bit markers, cross-byte reads, full-width fields, and every alignment;
- FSE distribution construction, low-probability symbols, transitions, and
  bounded weight-stream termination;
- overlapping history expansion at offsets 1 through 16 and invalid history;
- match-table collisions, reset, and stale positions;
- the maximum 11-bit Huffman tree with long and short terminal codes;
- encoding transitions for every predefined-table symbol/successor pair;
- the forward writer's zero-width and full-width fields.

These are targeted interpreter checks, not a claim that the entire corpus was
run under Miri. `.github/workflows/ci.yml` repeats the targeted unsafe checks on
Linux, but remote CI has not been run in this local session.

## Robustness and limits

Deterministic tests cover random bytes, structured frame-header mutations,
entropy-body bit flips, truncations, resource limits, table reuse, and frame
boundaries. An additional temporary entropy audit tested 266,240 pseudorandom
inputs in both safe and unsafe entropy builds without a panic; the checked-in
malformed suite is smaller so the backend matrix remains fast.

Remaining evidence gaps include a long-running coverage-guided fuzz campaign,
32-bit target execution, non-Apple platform execution, allocator-failure
injection, dictionaries, and streaming. Tests do not establish support for
unimplemented features or performance parity with upstream.
