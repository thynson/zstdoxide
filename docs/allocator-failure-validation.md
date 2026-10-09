# Allocation failure and ownership coverage

This is an index of executed Module 9 failure campaigns, not a claim that every
possible input or allocator implementation has been enumerated. Each campaign
first observes the requests of its successful workload, then rejects each
observed positive-size ordinal in a fresh run. Additional focused tests fail
growth or final-copy requests in pre-existing state. All production allocator
paths are now connected to public APIs; isolated leaf and aggregate checks
remain useful for their different failure invariants.

| Owner or operation | Verified failure and recovery boundary | Execution evidence |
| --- | --- | --- |
| Complete output and ring history | Failed growth preserves initialized old bytes, allocator identity and usable state; ring checks include wrap and window changes | [History](evidence/2026-10-09-allocator-history.json), [ring history](evidence/2026-10-09-allocator-ring-history.json) |
| Bit writers, FSE and Huffman storage | Reservations precede writes; failure releases weights/descriptions/tables and does not expose an invalid relation or description | [Inverse FSE](evidence/2026-10-09-allocator-inverse.json), [Huffman](evidence/2026-10-09-allocator-huffman.json), [Huffman construction](evidence/2026-10-09-allocator-huffman-builder.json) |
| Literal and sequence candidates | Failed preparation preserves committed tables/repeats; candidate commit swaps complete owners, and failed dictionary inverse preparation publishes nothing | [Literals](evidence/2026-10-09-allocator-literals.json), [sequence context](evidence/2026-10-09-allocator-sequence-context.json) |
| Match parsing and block encoding | Output reservation failures preserve initialized output; resolver errors retain documented partial scratch; failed seed rebuild requires reset before reuse | [Parser](evidence/2026-10-09-allocator-match-parser.json), [block encoding](evidence/2026-10-09-allocator-block-encoder.json) |
| Streaming contexts | Every observed request in selected plain/prepared-dictionary and strategy workloads returns AllocationFailed when rejected; context remains poisoned until reset, then reproduces expected bytes | [Decoder](evidence/2026-10-09-allocator-stream-decoder.json), [encoder](evidence/2026-10-09-allocator-stream-encoder.json) |
| Returned whole-buffer output | Failure drops partial output and all scratch; success leaves the original allocator with the only result allocation, independent of dictionary/input lifetimes | [Plain decoding](evidence/2026-10-09-allocator-whole-decode.json), [dictionary collection](evidence/2026-10-09-allocator-dictionary-collection.json), [compression](evidence/2026-10-09-allocator-whole-encode.json) |
| Dictionary preparation and training | Every observed preparation/training/finalization ordinal is rejected separately, including final validation-copy requests; malformed input drops partial owners, and temporary raw content is released before final return | [Dictionary](evidence/2026-10-09-allocator-dictionary.json), [raw training](evidence/2026-10-09-allocator-raw-training.json), [formatted training](evidence/2026-10-09-allocator-formatted-training.json) |

The whole-buffer compression sweep alone covers 106 observed request ordinals
per configuration: empty no/raw/prepared-dictionary paths have 1/2/8 requests;
nonempty paths have 29/26/40. These are workload-specific observations, not API
promises about future allocation counts. Every selected run checks that the
expected failure was actually reached and that final drop releases storage.
The exact test names, workload sizes, feature selections and logs are retained
in the linked checkpoint records. The complete codec feature matrix is recorded
separately from these focused campaigns.

Contexts deliberately have different failure contracts. Decoding may update an
earlier sequence channel or append provisional output before a later error.
Streaming errors therefore require reset; they do not promise rollback of all
accepted input or touched output. Encoding publishes candidate entropy only
after accepting the corresponding block. Dictionary seed setup can mutate a
private seed before a later inverse-table request fails, so its caller must
reset and complete setup before reuse. Tests exercise these boundaries rather
than incorrectly requiring every operation to be transactional.

The ledger tracks returned layouts consistently, including growth failure and
distinct clone identities. Old allocations remain with their actual owners;
moving or swapping a context/table also moves its allocator. Borrowed non-Clone
allocators are covered through standard reference implementations. Caller
source/output buffers, test ledgers and external C allocations are outside the
codec payload domain. Standard `into_shared` additionally performs its documented
infallible Global allocation for counts and the inline dictionary object.
Allocator callbacks and ordinary Clone retain their own panic/allocation policy.

Strict-provenance Miri checks cover selected ownership, bounds and failure paths.
Full ordinal sweeps generally run in normal safe/feature configurations; records
identify bounded Miri selections explicitly. The independent
[source audit](evidence/2026-10-09-allocator-ownership-audit.json) found no open
production routing, fallibility, detached-owner, or unsafe-proof issue in the
recorded final source. Its documentation finding about infallible shared-object
allocation was corrected. That review is additional source evidence, not another
dynamic run or a universal proof about untested allocators and targets.
