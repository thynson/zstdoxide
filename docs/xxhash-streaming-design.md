# Incremental XXH64 design

Reference: facebook/zstd v1.5.7 `lib/common/xxhash.h`, especially
`XXH64_state_t`, `XXH64_reset`, `XXH64_update`, and `XXH64_digest`.

## Ownership and invariants recorded before implementation

The existing `checksum(&[u8]) -> u64` hashes a complete slice with seed zero.
Its four lane accumulators are local scalars; whole 32-byte stripes feed those
lanes, then the remaining bytes feed the finalization rounds. The frame format
stores the low 32 bits, while this module preserves the full 64-bit hash API.

`XxHash64` owns four `u64` lane accumulators, a modular `u64` byte count, a fully
initialized `[u8; 32]` tail, and a tail-length index. It has no allocation,
borrowed pointer, saved input reference, or self-reference. Methods accept
ordinary borrowed input slices only for the duration of the call.

After every update, `tail_len < 32`; the initialized live tail is
`tail[..tail_len]`, and `tail_len` equals the total byte count modulo 32. Every
complete stripe contributes exactly once to all four lanes. Hash and length
arithmetic intentionally wraps, matching the upstream unsigned arithmetic.
The digest starts from combined lanes when the modular count is at least 32,
otherwise from lane 2 plus the short-input constant, exactly as upstream does
(lane 2 is zero for ordinary short seed-zero inputs).

An update first fills a pending stripe, processes complete borrowed input
stripes directly, and copies only the final incomplete stripe. Every access
uses safe slices and fixed-size arrays. The scalar state and tiny inline tail
do not require a separate unsafe backend: introducing unchecked indexing would
create an artificial alternative rather than an ownership/access-family design.
The state remains `no_std`, allocation-free, and independent of allocators.

`digest(&self)` finalizes a local hash value without consuming or modifying
state. `reset(&mut self)` restores the initial seed-zero state. Empty updates
are no-ops, and callers may inspect any prefix digest before appending more
bytes. `checksum` retains its name, signature, and seed-zero full-width result;
its one-shot stripe iteration will share only the lane-combination/finalization
helpers so the established whole-buffer path remains direct.

## Work decomposition

1. Add the incremental state and safe update/digest/reset API, reusing precisely
   the existing mixing/finalization operations while preserving one-shot calls.
2. Verify every prefix length 0..=512 and arbitrary update boundaries against
   independently generated upstream digests, plus large multichunk vectors.
3. Verify nonmutating digest, empty updates, reset/reuse and unaligned slices;
   run focused safe/all-feature/release tests, Clippy, and no_std compilation.

## Implemented API and proof boundaries

The internal API is now:

```rust
XxHash64::new() -> XxHash64
XxHash64::update(&mut self, input: &[u8])
XxHash64::digest(&self) -> u64
XxHash64::reset(&mut self)
```

`Default` initializes the same seed-zero state; `Clone` creates an independent
snapshot. The one-shot `checksum` continues to return `u64` and directly
process borrowed stripes. `mix_stripe`, `combine_lanes`, and `finish` are shared
safe helpers. All existing one-shot vectors continue to pass.

The pending-tail update compares input length against `32 - tail_len` before
adding lengths, so it never relies on a possibly overflowing
`input.len() + tail_len` expression. Filling a tail copies exactly that missing
suffix; complete input stripes come from `chunks_exact(32)`, and the retained
remainder is strictly shorter than 32 bytes. The finalizer reads only that live
remainder. No input alignment, padding, hidden capacity, or caller lifetime
beyond the update call is assumed.

The length counter deliberately preserves upstream's wrapping behavior even at
its theoretical 2^64-byte boundary. In the short-count digest branch, lane 2
plus PRIME_5 matches upstream exactly; lane 2 is the zero seed until the first
stripe has been processed. Ordinary Zstandard frames stay within their declared
content-size/resource limits before this theoretical boundary matters.

## Independent verification

A temporary C oracle compiled the pinned `lib/common/xxhash.h` directly, with
`XXH_STATIC_LINKING_ONLY` and `XXH_IMPLEMENTATION`. It independently generated
513 full digests for prefixes 0..=512 of bytes `(i*37+11) mod 256`. Those expected
values are embedded in the Rust tests. Every prefix is checked with every
two-way split: **131,841 split cases**, with empty updates and non-consuming
prefix/final digest calls between updates.

A second oracle input contains 1,000,003 bytes from this wrapping `u32` generator:
start at `0x6a09e667`, update with `state*1664525+1013904223`, emit the high byte.
Both upstream one-shot XXH64 and upstream incremental XXH64 independently return
`0x2203bab47f5a29b7`. Rust matches it for chunk sizes 1, 7, 31, 32, 33, 127, 1024,
and 65535, plus an irregular schedule including empty updates. Additional tests
cover unaligned backing offsets 0..15, cloned prefix snapshots, repeated digest,
and reset/reuse with dirty lanes and tails.

Focused validation covers all seven checksum tests in safe debug, all-feature
debug, and all-feature release builds, and the crate compiles without default
features. The incremental type is initially unused by the non-test library
until the streaming consumer is integrated, so ordinary library checks may
report expected dead-code warnings during that intermediate work item. No
warning suppression was added. The coordinating streaming module's completion
check must run warning-free full Clippy after integrating the state.
