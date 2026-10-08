# Tasks

## Module 5: bounded streaming and reusable contexts

- [ ] Specify input/output progress, completion, reset, and error contracts.
- [ ] Analyze ring-history ownership, wraparound indices, and backend invariants.
- [ ] Implement and test safe bounded ring history independently of frame logic.
- [ ] Add an independently gated unsafe ring-history backend and compare it.
- [ ] Implement incremental XXH64 state with arbitrary update boundaries.
- [ ] Parse partial frame/block headers with bounded staging and explicit phases.
- [ ] Resume block decoding with caller-provided output slices and bounded history.
- [ ] Preserve entropy/repeat state and checksums across input/output exhaustion.
- [ ] Add a streaming encoder with bounded block staging and finish semantics.
- [ ] Test arbitrary chunk sizes, concatenation, truncation, reset, and oracle parity.
- [ ] Measure retained memory and safe/unsafe throughput; document stream limits.

Completed foundation, entropy, one-shot decoding, initial encoding, and validation
checklists were cleared at their module boundaries. See `docs/progress.md` and
the small work-item commits for completed work.

## Roadmap

1. Bounded streaming and reusable contexts.
2. Raw and prepared dictionaries, dictionary IDs, and training.
3. Huffman/adaptive FSE encoding and improved compression strategies.
4. Allocator parameterization and broader platform validation.
5. Advanced compression levels, long-distance matches, threading, and adapters.

Each module must be decomposed before implementation. Completion of an initial
codec milestone does not mean parity with the upstream library or CLI.
