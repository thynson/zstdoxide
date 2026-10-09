# Focused Module 9 platform validation

This report covers committed source `dac4a43` in the isolated archive snapshot
`/tmp/zstdoxide-module9-platform-dac4a43`. The 44 recorded codec, helper, Cargo,
and toolchain source/config files match that commit byte for byte. The compiler
is the repository pin, `nightly-2026-10-08`, reporting Rust 1.101.0-nightly
(`1d81eb4ad9cd207e3e638bd32b17ec4fce8412a6`). No repository sources, toolchain
configuration, installed targets, or system tools were changed.

## Scope and execution modes

| Target | Width / byte order | Mode | Scope |
| --- | --- | --- | --- |
| `i686-unknown-linux-gnu` | 32-bit / little-endian | Miri cross-interpretation, strict provenance | Ten exact existing tests safe; nine backend-sensitive tests all features. All selected tests passed. |
| `s390x-unknown-linux-gnu` | 64-bit / big-endian | Miri cross-interpretation, strict provenance | The same ten-safe/nine-all selection. All selected tests passed. |
| `thumbv6m-none-eabi` | 32-bit / little-endian, no pointer atomics | Offline library compilation and caller code generation | Safe no_std library check; safe/all-12-unsafe-feature no_std caller probe with explicit `Rc` type equality. All passed. |

The two Linux runs interpret Rust MIR for the selected targets. They are neither
native execution nor QEMU, and do not exercise a real foreign kernel, ABI linker,
CPU instruction stream, or hardware allocator. The bare-metal target has no
runtime execution, linker image, global allocator implementation, or memory-fit
claim. Its probe emits an rlib and instantiates the public APIs with `Global`.
The generic caller separately type-checks arbitrary `A: Allocator + Clone`, but
it does not run a custom allocator on bare metal.

## Selected contracts

The ten distinct names are recorded in the machine-readable evidence. Each target
runs all ten in the safe build, and nine in the all-feature build. XXH64 is plain
shared safe code without an unsafe feature; after its full safe run passes, the
identical expensive vector is omitted from the all-feature repeat. The original
all-feature batches were stopped during that duplicate XXH64 test at the
coordinator's request, and the nine remaining tests were rerun to completion.
Those interrupted batches are retained as diagnostics, not successful full runs.
The completed selected batches total 38 test executions; two preliminary wide
history checks are separately recorded. Selected contracts cover:

- Nonempty little-endian cursor reads and rejected `usize::MAX` requests.
- Unaligned reverse-bitstream reads with full 32-bit fields.
- All 23 stored C XXH64 vector lengths, including zero, stripe/tail boundaries,
  and the existing 1 MiB maximum input.
- Ring-history positions above `u32::MAX`, a wrap, and the `u64::MAX` boundary.
- Injected custom-allocator growth failure preserving wrapped history and later
  usable matches.
- Borrowed FSE views preserving the distinction between RLE and a normalized
  singleton, checked extreme indices, and no table-copy allocation.
- Borrowed Huffman views with independent non-Clone output ownership,
  allocation failure recovery, and exact one/four-stream output.
- Four observed streaming-decoder allocation failure ordinals, poisoning,
  reset recovery, fragmented input/output, and an independently owned shared
  dictionary surviving all attempts.
- Standalone entropy caller-capacity formulas for both native word sizes.
- Raw/formatted dictionary indexed access and rejected `usize::MAX` positions.

These are focused Rust ownership/platform assertions. They do not claim the
original 32-bit fuzzer durations or QEMU CLI campaigns in upstream
`tests/Makefile`, and they do not upgrade unrelated upstream test coverage.
Allocation counts are checked within each target's selected tests, not asserted
equal across architectures.

## Linux test allocator no-unwind review

The exact installed standard-library source selects the inline futex mutex for
both Linux targets (`std/src/sys/sync/mutex/mod.rs`). Its constructor is a const
atomic initialization; uncontended lock/unlock uses atomic compare-exchange
and swap, without heap allocation, native lazy initialization, or panic branches
(`std/src/sys/sync/mutex/futex.rs`). The tracking helper owns a private mutex,
exposes no guard, invokes no caller callbacks while holding it, and does not
recursively acquire it. Each selected test uses its own ledger and one test
thread; all guards unlock on their locking thread before owner destruction.
Poisoning is recovered by the helper. The reviewed source hashes are in the
capability evidence. This review supplements the earlier macOS warm-up proof;
it is not an assertion that all untested stdlib mutex backends behave alike.

## Offline target preparation and no-atomics probe

Each Miri sysroot was built with the pinned local `rust-src` and cached Cargo
packages using `cargo miri setup --target TARGET --offline`; explicit
`MIRI_SYSROOT` and `CARGO_TARGET_DIR` keep artifacts under `/tmp`.
The installed Miri README explicitly documents cross-interpretation and lists
s390x as its supported big-endian target. Target cfg output confirms the exact
width, byte order, and atomics used here.

The temporary probe at `/tmp/zstdoxide-platform-rc-probe/src/lib.rs` rejects any
target with pointer atomics using `compile_error!`. It gives the shared value
the explicit type `Rc<Dictionary<&A>>` and converts it to `SharedDictionary<&A>`.
It constructs an allocator-aware decoder, processes caller slices, resets,
attaches a prefix, changes options, and calls `decompress_in`. A non-generic
entry point instantiates these paths with `Global`; the dictionary temporarily
borrows the factory, while returned output owns the original factory value.
The selected caller is compiled both with no unsafe features and with all 12
unsafe features; it does not enable `std` or the test allocator dependency.

Exact commands, source/config hashes, target cfg logs, setup logs, result logs,
and probe source/Cargo files are preserved in the final JSON evidence. This is a
checkpoint of `dac4a43`, not a claim about later allocator migration changes or
completion of the entire Module 9 platform matrix.

The [machine-readable record](evidence/2026-10-09-allocator-platform.json) preserves
all exact commands, probe sources, result summaries, and source/log hashes.
