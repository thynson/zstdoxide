# Upstream allocation and workspace test plan

This is a read-only source mapping prepared for Module 9. The reference is
facebook/zstd **v1.5.7**, revision
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`. All source paths and line numbers
below are relative to that upstream checkout. They are not paths to a vendored C
implementation in this repository.

Allocator parameterization can cover ownership and allocation instrumentation.
Static workspaces, size estimators, automatic shrinking, stable-buffer promises,
and stack budgets have additional contracts and require separate APIs or policies.
No coverage status should be upgraded on the basis of allocator support alone.

## Custom allocation callbacks and counter schedule

`tests/fuzzer.c:148-307` contains the principal custom-allocation exercise.

- Lines 152-160 define `mallocCounter_t`: total requested bytes, current bytes,
  peak bytes, allocation count, and free count, all initially zero.
- Lines 162-174 implement `FUZ_mallocDebug`. Failed `malloc` returns before any
  counters are changed. Successful allocation credits the requested size and
  increments the allocation count.
- Lines 177-184 implement `FUZ_freeDebug`. The free count is incremented and
  macOS `malloc_size(address)` is subtracted before freeing. The allocation and
  free sides therefore use different size measures. A Rust ledger should use
  consistent layout/requested-size accounting instead of copying this asymmetry.
- Lines 186-192 report peak bytes, allocation count, and total bytes. The loops
  do not assert fixed counts, balanced counters, or decoded-content equality.
- Lines 194-278 implement `FUZ_mallocTests_internal`; it requires display level
  at least three. Lines 5124-5126 arrange that verbosity for the CLI memtest.
- Lines 279-298 allocate a source of exactly **89,456,640 bytes**
  (`64 MiB + 16 MiB + 4 MiB + 1 MiB + 256 KiB + 64 KiB`) and an output of
  `ZSTD_compressBound(source_size)`. Those two buffers are outside the tracked
  context allocator. The source uses `RDG_genBuffer` with literal-distribution
  parameter `0.0`, the selected compressibility, and the selected seed.

The exact configuration schedule is:

| Source | Condition | Configuration and operation |
| --- | --- | --- |
| `tests/fuzzer.c:207-218` | `part <= 1` | Levels 1 through 6; create custom CCtx, one-shot `ZSTD_compressCCtx`, free, report. |
| `tests/fuzzer.c:220-235` | `part <= 2` | Levels 1 through 6; create custom CStream, initialize, one `ZSTD_compressStream`, one `ZSTD_endStream`, free, report. |
| `tests/fuzzer.c:237-253` | `part <= 3` | Workers 1 through 4 crossed with levels 1 through 6; custom CCtx, set level/workers, `ZSTD_compress2`, free, report. |
| `tests/fuzzer.c:255-274` | `part <= 4` | The same 24 combinations; one Continue call followed by End calls until completion, free, report. |

`--memtest=N` selects a suffix of these groups, not a single group. Values 1, 2,
3, and 4 execute respectively **60, 54, 48, and 24 configurations**. Bare
`--memtest` selects 1 (`tests/fuzzer.c:5046-5048`). Default compressibility is 50
percent (`tests/fuzzer.c:54`). An unspecified seed can be derived from time
(`tests/fuzzer.c:5023,5113-5116`), so reproducible Rust campaigns should record
explicit seeds rather than assume the original seed is always zero.

This implementation is macOS-only. `tests/fuzzer.c:303-307` is a no-op on other
platforms and is not evidence of allocator coverage.

**There is no deliberate fail-at-N allocation schedule in these loops.** A Rust
fail-at-N campaign, exact ownership balance, leak-free drop, allocator-identity
checks, and full decoded-content comparison would be additional Rust assertions.
They should be documented as such, not attributed to this upstream test.

The exact large source can be preserved in an opt-in Rust workload. Normal tests
can use smaller sources for the additional ownership invariants. Supported Rust
strategies can exercise one-shot and streaming contexts, while numerical levels
1 through 6 and the worker groups remain explicit qualifications.

## Context size and allocator helper distinctions

`tests/fuzzer.c:811-817` creates a DCtx with default custom-memory parameters,
requires a non-null context and nonzero `ZSTD_sizeof_DCtx`, then frees it. It is
not a non-default allocator callback test. Rust `retained_capacity()` is not the
same as C whole-context size: an inline, lazily allocating Rust context may
legitimately retain zero heap bytes. A whole-object footprint API, if wanted,
would need its own definition.

`tests/fuzzer.c:723-729` prints CCtx size after level-1 compression; it does not
assert a specific byte size. Function-pointer and symbol-ABI assertions starting
at `tests/fuzzer.c:820` are C ABI obligations rather than allocator contracts.

`tests/fuzz/fuzz_helpers.c:16-38` provides allocation helpers that assert success
for nonzero requests and return null for zero-sized requests. The randomized
helper can consume a selector on zero-sized input but still returns null; its
comment must not be interpreted as an actual random-pointer return. There is no
failure injection in these helpers. Lines 42-47 avoid a read for zero-length
comparison.

## Automatic reduction requires a separate policy

`tests/fuzzer.c:968-1005` compresses one 200-byte frame followed by **256 ten-byte
frames**, then supplies the compressed stream to a decoder **one byte per call**.
It inspects context size after each call and requires at least one decrease.

`tests/fuzzer.c:1929-1959` initializes a level-19 context for unknown-size
bufferless compression and compresses one byte. A separate level-1 context also
compresses one byte. The initial large context must exceed the small context by
**32 times**. The large context then receives at most **1,100** further one-byte
level-1 compressions; its reported size must eventually decrease.

Neither assertion follows from allocator routing. They require an automatic
shrinking or reclamation policy. Current Rust reset behavior that deliberately
retains high-water capacity should remain explicit until a separate policy is
designed.

## Static workspace failure and recovery schedule

`tests/fuzzer.c:1964-2190` tests caller-provided storage and size estimates at
level 4. The large source is the **5 MiB** `CNBuffer` defined at line 652.

1. Lines 1977-1991 estimate a context for a 32 KiB source, initialize static
   compression/decompression contexts, and successfully compress 32 KiB.
2. Lines 1995-1999 compress the full 5 MiB source with that small context and
   require the specific `memory_allocation` error.
3. Lines 2003-2012 reinitialize the same backing storage with the full CCtx
   extent and require successful compression of the large source.
4. Lines 2016-2024 run **130** small compressions and then require the workspace
   still to support the large input. The original duration is
   `ZSTD_WORKSPACETOOLARGE_MAXDURATION + 2`; duration 128 and factor 3 are defined
   in `lib/common/zstd_internal.h:258,265`.
5. Lines 2028-2047 begin level-4 compression, compress the large source, and
   require static decoding to produce the exact length and bytes.
6. Lines 2050-2056 require maximum-level initialization to fail, then require
   level-1 initialization to work again in the same storage.
7. Lines 2059-2064 initialize a stream in a CCtx-only extent without allocating;
   the first compression call must fail. Lines 2068-2083 reinitialize with the
   CStream-sized extent and require streaming at levels 4 and 1 to work.
8. Lines 2086-2088 require static stream initialization with a **64 KiB
   dictionary** to fail.
9. Lines 2091-2096 initialize a stream in a DCtx-only extent, then require the
   first decompression call with `ZSTD_FRAMEHEADERSIZE_MAX + 1` input bytes to
   fail.
10. Lines 2100-2138 construct estimated CCtx/CStream workspaces from default
    context parameters and require successful compression and exact decoding.
    Lines 2142-2184 repeat this with `maxBlockSize=0`.

A budget allocator can test an analogous resource-exhaustion principle. It does
not satisfy same-address initialization, caller-storage bounds, no-heap static
construction, or estimator guarantees. Full ports need separate workspace/arena
APIs.

Prepared dictionary workspaces add distinct obligations:

- `tests/fuzzer.c:2496-2505`: estimate and initialize a static DDict and decode
  using it.
- `tests/fuzzer.c:2788-2800`: estimate a by-reference CDict and report size.
- `tests/fuzzer.c:2826-2851`: levels **1 through 22**, estimated by-copy CDict
  workspace, static automatic dictionary initialization, and compression of up
  to 10 KiB from the source.

Allocator-owned `Dictionary` and `SharedDictionary` are not substitutes for the
caller-storage or by-reference lifetime portions of these cases.

## Estimator sweeps

- `tests/fuzzer.c:2192-2199`: estimated CCtx/CStream sizes at level -1 must be
  positive and no larger than level 1.
- `tests/fuzzer.c:4357-4369`: estimated CCtx size must be monotonic for levels
  1 through the level below the maximum.
- `tests/fuzzer.c:4373-4392`: levels **-3 through maximum**, dictionary sizes
  **0 through 256 KiB in 16 KiB increments**, and source sizes **2 KiB to below
  300 KiB in 2 KiB increments**; both estimates must be valid and the
  level-based estimate must bound the parameter-based estimate.
- `tests/zstreamtest.c:345,355,368,438,451,1107`: further stream sizing and
  accounting assertions.
- `examples/streaming_memory_usage.c:58-134`: compress `abcde` into a 128-byte
  buffer with unknown source size, optional window override, and a level
  sweep; decode it; require actual compression/decompression context sizes to
  fit their estimates at lines 116-124.

These require a whole-context footprint model and estimator APIs. Retained heap
capacity is useful telemetry but is not this contract.

## Stable buffers and direct allocation promises

`tests/zstreamtest.c:699-720` uses stable output for an empty frame and zero-sized
output and requires DCtx size to remain unchanged. Empty-frame decoding itself
already has partial coverage; stable-buffer allocation behavior is separate.

The fuller stable-output schedule is:

- Lines 786-808: decode the complete large frame in a single pass, check exact
  bytes, and require no increase in context size.
- Lines 811-822: supply input in random increments of **1 through 16 bytes**
  while output remains stable; verify exact bytes and increased context size
  for the input buffer.
- Lines 825-834: the complete source with stable output one byte too short must
  produce `dstSizeTooSmall`.
- Lines 838-850: first omit the last compressed byte, then supply the full
  input but reset output position to zero; require `dstBufferWrong`.
- Lines 854-866: disable stable output and again fragment input by 1 through
  16 bytes; the allocated output buffer must increase context size.

Lines 725-779 also vary maximum block size between 2 KiB and the full bound,
reject a 1 KiB limit, and assert specific memory differences: the streaming-size
difference is `3 * (128 KiB - 2 KiB)`, and the full streaming context is at least
`2^18 + 3 * 128 KiB` larger than its single-pass counterpart. These are
block-limit and buffering-policy contracts, not just allocator injection.

Lines 884-1107 test stable input/output for compression, including pointer
identity and modification constraints. Lines 2360-2470 combine static contexts
with an external sequence producer. Those API combinations remain separate.

## Kernel workspace and stack contracts

`contrib/linux-kernel/test/static_test.c:31-44` estimates a DCtx workspace,
initializes it statically, and decodes the exact 13-byte checksummed empty frame
with null output and capacity zero. The data/empty-output behavior is already
partially ported; static construction is not.

`contrib/linux-kernel/test/test.c` contains these additional contracts:

- Lines 56-143: Btrfs-style sources up to 128 KiB, levels **-1 through 15**,
  asserted window log at most 17, one workspace sized to the maximum of the
  compression/decompression bounds, and exact **4,096-byte** chunks with full
  byte equality.
- Lines 145-173: level-19 static compression followed by static decompression.
- Lines 176-180: minimum level below zero and maximum level exactly 22.
- Lines 183-218: C stack-poison machinery and a **2,560-byte** maximum stack-use
  assertion. This requires separate instrumentation; the C dangling-stack
  mechanism must not be copied into safe Rust.

`contrib/linux-kernel/zstd_deps.h:39-40` makes ordinary allocation fail, forcing
custom-memory or static contexts. Rust `no_std + alloc` does not mean that the
codec operates without heap allocation.

## Platform execution and tooling

- `tests/Makefile:172-174,184-189` builds 32-bit fuzzer/streaming binaries.
  Lines 384-385 and 392-393 execute them, normally for **200 seconds** and
  **90 seconds** respectively (defaults at lines 79-80).
- `tests/Makefile:192-204` has ASan, TSan, and UBSan targets. Lines 279-292 run
  Valgrind with full leak checks and error exit status; the schedule includes
  CLI exercises, a one-minute fuzzer run, and a short benchmark run. Rust can
  preserve the ownership/UB validation intent while CLI coverage remains a
  separate obligation.
- `.github/workflows/dev-short-tests.yml:456-525` runs QEMU configurations
  including ARM, AArch64, PPC, PPC64LE, S390X, MIPS, RISC-V, M68K, and SPARC.
  Its `make check` maps to the CLI smoke target (`tests/Makefile:316-317`),
  **not the complete fuzzer suite**.
- Root `Makefile:238-250` separately invokes `tests/fuzztest` for cross-platform
  targets with `--no-big-tests`; `tests/Makefile:323` includes fuzzer,
  zstreamtest, and decodecorpus in that target.
- `.github/workflows/dev-long-tests.yml` includes 32-bit big tests, sanitizer
  jobs, ARM fuzzing, and Valgrind jobs.

A Rust plan should distinguish successful cross-compilation from actual target
execution. Useful initial execution targets are one 32-bit little-endian target
and one big-endian target, with safe and unsafe configurations and deterministic
format/content checks. Allocation capacity totals need not match across pointer
widths or alignments. `no_std` and targets without pointer atomics also deserve
explicit checks where supported, especially for shared dictionary ownership.

## Current inventory boundary

The custom callback/accounting anchors, static workspace groups, estimators, and
shrinking groups are blocked in the current manifest. Existing ordinary
streaming and empty-frame ports give partial coverage to relevant stable-buffer
and kernel entries, not complete workspace coverage. C build orchestration and
ABI-only assertions are classified separately as non-applicable.

This source review alone does not justify changing any status. New allocation
support must be linked to executable assertions before a source group advances.

## Proposed independently verifiable checkpoints

1. **Allocation-domain design.** Record the allocator for codec buffers, history,
   entropy tables, match structures, dictionary/shared ownership, training
   workspaces, and returned output. Define allocator lifetime, layout accounting,
   reset/drop behavior, error recovery, and requested/capacity/inline/peak memory
   distinctions. Do not assume a future Rust `Allocator` API is already stable.
2. **Test ledger and failure instrumentation.** Cover allocation, grow, shrink,
   free, alignment, zero-sized layouts, overflow, allocator identity, and exact
   cleanup. Add deterministic fail-at-N schedules based on observed allocation
   events. Label balanced ownership and failure injection as additional Rust
   assertions, not original C counter assertions.
3. **One storage family per implementation checkpoint.** Start with a leaf
   buffer and transactional growth failure, then history and stream buffers,
   entropy structures, match structures, dictionaries, and training. Preserve
   the same allocation contract in safe and all independently gated unsafe
   representations.
4. **Decoder integration and recovery.** Exercise empty/raw/RLE/compressed,
   skippable, and dictionary cases; fail each observed allocation site; verify
   cleanup, poisoned-state behavior, reset recovery, and ownership of returned
   one-shot output.
5. **Encoder integration and transactional publication.** Exercise Greedy,
   Chain, and Lazy; inject failures during HUF/FSE candidate construction,
   history growth, and output reservation; verify that failed candidates do not
   publish invalid state and reset recovery preserves format correctness.
6. **Shared dictionaries and training.** Check allocator lifetime through shared
   handles, final-owner cleanup, failed parsing/training cleanup, and isolation
   of allocator domains. Retain static-storage and by-reference qualifications.
7. **Original custom-context workload.** Preserve the exact 89,456,640-byte RDG
   source as an opt-in campaign, with explicit seed and supported-policy
   adaptation. Keep numerical level and worker qualifiers. Use smaller normal
   tests for the added Rust ownership invariants.
8. **Platform execution.** Run meaningful 32-bit and big-endian cases, separate
   compile-only portability checks, and allocator failure tests under Miri or
   appropriate tooling. Compare codec bytes/content where deterministic, not
   architecture-dependent allocation totals.
9. **Separate future API groups.** Keep static arenas, estimators, shrinking,
   stable buffers, stack budgets, and multithreading separately decomposed. Do
   not present a budget allocator as completing those contracts.
