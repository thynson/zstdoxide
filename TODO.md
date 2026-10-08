# Tasks

## Module 1: entropy primitives

- [x] Document entropy structure invariants and module work decomposition.
- [x] Implement checked reverse bitstreams and the unsafe-bitstream alternative.
- [x] Implement FSE normalized-count parsing and decoding tables with unsafe-fse.
- [x] Implement two-state FSE decoding of Huffman weights.
- [x] Implement Huffman table construction and safe/unsafe-huffman decoding.
- [x] Implement raw, RLE, compressed, and treeless literal sections.

## Module 2: frames and sequence execution

- [x] Document frame, cursor, history, and sequence invariants.
- [x] Implement checked frame headers and safe/unsafe-byte-cursor access.
- [x] Implement bounded history and safe/unsafe-history match expansion.
- [x] Implement XXH64 content checksum with published vectors.
- [x] Implement sequence decoding and repeat-offset updates.
- [ ] Integrate frame decoding, concatenation, and skippable frames.

## Module 3: initial compressor

- [ ] Document encoder structures and work decomposition.
- [ ] Encode standard checksummed frames using raw and RLE blocks.
- [ ] Implement indexed match tables with safe/unsafe-match-table backends.
- [ ] Encode greedy LZ sequences with predefined FSE tables and raw fallback.

## Module 4: validation and performance

- [ ] Add independent official zstd interoperability fixtures and tests.
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
