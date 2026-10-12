# Reference Fast, DoubleFast and Greedy profiles

The four reference compression entry points are separate from the existing
named Greedy/Chain/Lazy encoder. They encode complete borrowed inputs without a
dictionary, using the pinned v1.5.7 Fast, DoubleFast or Greedy sampling and repeat-offset
algorithm.
Huffman and sequence entropy choices are independent Rust implementations;
compressed bytes, block decisions and ratios need not equal the C encoder.

## Levels and frame flags

```rust
use zstdoxide::{compress_at_level, decompress, FrameHeader};

let input = b"whole input Fast, whole input Fast, whole input Fast";
let frame = compress_at_level(input, -1)?;
let (header, _) = FrameHeader::parse(&frame)?;
assert_eq!(header.content_size, Some(input.len() as u64));
assert!(!header.has_checksum);
assert_eq!(header.dictionary_id, 0);
assert_eq!(decompress(&frame, input.len())?, input);
# Ok::<(), zstdoxide::Error>(())
```

Admission follows the resolved strategy, including actual source-size
adjustment. It is not a fixed level allowlist:

| Request | Current execution |
| --- | --- |
| Level 1 | Fast for every source tier |
| Negative level | Fast with the pinned acceleration and raw literal policy; values below the minimum level normalize to that minimum |
| Level 2 | Fast except for source sizes 131073..=262144, which execute DoubleFast |
| Levels 0 and 3 | Identical DoubleFast execution for all source tiers |
| Level 4 | DoubleFast for sizes 16385..=131072 and above 262144; Greedy otherwise |
| Other resolved strategy | `Error::UnsupportedFeature`; no replacement algorithm |

Frames carry the exact source size, no checksum and no dictionary identifier.
When input fits the selected window, a single-segment frame advertises that
input size. Otherwise an explicit power-of-two window descriptor advertises the
resolved window. Blocks are at most the smaller of that window and 128 KiB.
Empty input still has one final empty raw block and allocates no search workspace.

The existing `compress`, `Encoder`, `EncoderOptions` and named strategy defaults
keep their established behavior. Numeric streaming setters, dictionaries,
long-distance matching and workers are not provided by these reference APIs.

## Explicit requests

```rust
use zstdoxide::{CompressionParameterRequest, compress_with_parameters, decompress};

let input = b"indexed Fast history, indexed Fast history";
let mut request = CompressionParameterRequest::with_level(1);
request.set_window_log(17)?;
request.set_hash_log(14)?;
request.set_min_match(5)?;
let before = request;
let frame = compress_with_parameters(input, &request)?;
assert_eq!(request, before);
assert_eq!(decompress(&frame, input.len())?, input);
# Ok::<(), zstdoxide::Error>(())
```

The source is always known, including known empty input, and dictionary size is
zero. A source hint cannot override the actual length. Resolution and strategy
admission happen before allocator cloning or allocation. A valid unsupported
strategy returns `UnsupportedFeature` before algorithm-specific resource checks.

Effective Fast limits are window logs 10..=20 and hash logs 6..=18. DoubleFast
supports window logs 10..=21 and independent hash/chain logs 6..=18. Greedy
supports window logs 10..=21 and hash logs 6..=19. All three admit
minimum-match settings 3..=7 and target lengths 0..=131072. Larger otherwise
valid implemented resources return `ResourceLimit`. The limits apply after
adjustment: an unadjusted large requested hash can still fit the admitted range for tiny input.
Normal scalar setters continue to enforce their reference validity domains.

Minimum-match setting 3 uses the same four-byte key as setting 4; settings
5..=7 select wider keys while emitted matches still have a four-byte minimum.
For Fast, positive target length accelerates sampling and forces raw literal
sections, including bypassing literal RLE. Whole block RLE remains a separate decision.
Chain/search fields are validated but do not create a chain or visit budget for
Fast. The sampled table stores `2^hash_log` usize positions into the borrowed
whole source, retained across raw, RLE and compressed block decisions.

DoubleFast owns two independent tables: `2^hash_log` positions for eight-byte
long keys and `2^chain_log` positions for short keys. Its chain log sizes a hash
table, not a linked chain. Minimum-match 3 uses the same four-byte short key as 4;
settings 5..=7 widen that key. DoubleFast ignores search log and target length
after scalar validation, preserving normal Huffman/RLE literal selection.
Both tables retain sampled source history across block decisions.

```rust
use zstdoxide::{CompressionParameterRequest, ReferenceStrategy, compress_with_parameters};
let input = b"two tables, two tables, two tables";
let mut request = CompressionParameterRequest::with_level(3);
request.set_strategy(Some(ReferenceStrategy::DoubleFast));
request.set_window_log(21)?;
request.set_hash_log(18)?;
request.set_chain_log(16)?;
let frame = compress_with_parameters(input, &request)?;
# Ok::<(), zstdoxide::Error>(())
```

Greedy selects chains for resolved window logs through 14 and rows above 14.
The chain branch owns separate head/link arrays, uses chain log at most 18 and
search log at most 8, and visits at most `2^search_log` candidates. The row branch
owns separate position and byte-tag arrays of `2^hash_log` elements. It does not
allocate a chain or cap its unused log; valid search settings determine row width
and visits, capped at 64. Scalar row hashing uses the fresh-context salt on all
targets. Minimum-match settings clamp to key widths 4..=6. Target length is
validated but does not accelerate Greedy or disable literal compression.

The actual source size controls resolution even when a hint disagrees. For
example, level 4 selects chain Greedy on 16384 bytes, DoubleFast on 16385 bytes,
and row Greedy on 131073 bytes. An explicit Greedy strategy keeps Greedy but
still adjusts its dimensions before choosing chain versus row.

Candidate entropy tables and wire repeats become committed state only after
the complete compressed block is appended. Raw/RLE fallback keeps sampled
source history and discards pending entropy/repeats. Private contexts become
unusable after processing failure; public one-shot failure releases the whole
workspace and returns no partial frame.

## Allocator ownership

```rust
use std::alloc::Global;
use zstdoxide::{compress_at_level_in, compress_with_parameters_in,
    CompressionParameterRequest, decompress};

let input = b"owned output, owned output, owned output";
let request = CompressionParameterRequest::with_level(-3);
let first = compress_at_level_in(input, -3, Global)?;
let second = compress_with_parameters_in(input, &request, Global)?;
assert_eq!(first, second);
assert_eq!(decompress(&first, input.len())?, input);
# Ok::<(), zstdoxide::Error>(())
```

The returned `Vec<u8, A>` keeps the supplied allocator value. Each temporary
allocation retains its exact allocating clone, and all scratch is released
before return. Clones need not be interchangeable. Passing `&allocator` supports
non-Clone allocator values; the returned vector then borrows that allocator,
while never borrowing source or request. Collection allocation requests are
fallible; ordinary user-defined `Clone` may allocate or panic under its own
contract. See [allocator usage](allocator-usage.md) for owner lifetimes.

The core requires only `alloc`; filesystem, process and timing code remain in
test/oracle tooling. `unsafe-match-table`, `unsafe-bitstream`,
`unsafe-fse-encoder` and `unsafe-huffman-codebook` select the existing individual
storage backends. `unsafe-reference-chain` and `unsafe-row-table` independently
select the new Greedy leaves. The Fast/DoubleFast/Greedy parsers, persistent source states and
shared frame supervisor themselves forbid unsafe code and use the same checked contracts with every
backend combination.
