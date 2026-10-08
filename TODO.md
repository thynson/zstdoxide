# Tasks

## Module 3: initial compressor

- [x] Document encoder structures and work decomposition.
- [x] Encode standard checksummed frames using raw and RLE blocks.
- [x] Implement indexed match tables with safe/unsafe-match-table backends.
- [ ] Encode greedy LZ sequences with predefined FSE tables and raw fallback.

## Module 4: validation and performance

- [x] Add independent official zstd interoperability fixtures and tests.
- [ ] Verify Rust encoder output with the official decoder.
- [ ] Exercise malformed input, feature combinations, and no_std builds.
- [ ] Benchmark each data structure backend and whole-codec workloads.
- [ ] Document measured results, supported features, and remaining scope.

## Roadmap

1. Checked byte/bit access and entropy decoding (FSE and Huffman).
2. Frame/block decoding, history, checksums, and bounded allocation.
3. Interoperable frame encoding and initial compression strategies.
4. Official-codec differential tests and safe/unsafe benchmarks.
5. Streaming, dictionaries, advanced compression, and allocator support.

Each module must be decomposed before implementation. Completion of an initial
codec milestone does not mean parity with the upstream library or CLI.
