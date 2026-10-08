# Tasks

## Module 4: validation and performance

- [x] Add independent official zstd interoperability fixtures and tests.
- [x] Verify Rust encoder output with the official decoder.
- [x] Exercise malformed input, feature combinations, and no_std builds.
- [x] Add maximum-depth Huffman and malformed table regression cases.
- [x] Add a reproducible per-backend benchmark harness.
- [x] Record compressed sizes and full-output digests outside benchmark timing.
- [x] Benchmark each data structure backend and whole-codec workloads.
- [x] Document measured results, supported features, and remaining scope.

## Roadmap

1. Bounded streaming and reusable contexts.
2. Raw and prepared dictionaries, dictionary IDs, and training.
3. Huffman/adaptive FSE encoding and improved compression strategies.
4. Allocator parameterization and broader platform validation.
5. Advanced compression levels, long-distance matches, threading, and adapters.

Each module must be decomposed before implementation. Completion of an initial
codec milestone does not mean parity with the upstream library or CLI.
