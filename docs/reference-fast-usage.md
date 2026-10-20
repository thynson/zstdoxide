# Reference compression profiles

The four reference compression entry points are separate from the existing
named Greedy/Chain/Lazy encoder. They encode complete borrowed inputs without a
dictionary, using the pinned v1.5.7 Fast, DoubleFast, Greedy, Lazy, Lazy2, BinaryTreeLazy2, Optimal, Ultra or Ultra2 sampling, path and
repeat-offset algorithms within the documented resource limits.
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
| Level 5 | Lazy through 16384 bytes; Greedy otherwise |
| Level 6 | Lazy2 through 16384 bytes; Lazy otherwise |
| Level 7 | Lazy2 through 131072 bytes; Lazy above it; sizes above 262144 exceed resource caps |
| Level 8 | Lazy2; sizes above 262144 exceed resource caps |
| Level 9 | BinaryTreeLazy2 through 16384 bytes; Lazy2 through 262144; larger inputs exceed resource caps |
| Level 10 | Resource error through 16384 bytes (tree search log 9); Lazy2 for sizes 16385..=262144; larger inputs exceed resource caps |
| Levels 11/12 | Optimal for sizes 64..=2048; BinaryTreeLazy2 for 16385..=262144; resource errors elsewhere |
| Levels 13/14 | Ultra for sizes 64..=2048; other resolved implemented tuples exceed local caps |
| Level 15 | Resolved implemented strategies exceed local caps |
| Levels 16/17 | Ultra2 for sizes 64..=2048; other resolved tuples exceed local caps |
| Levels 18+ | Resolved strategies exceed local caps |
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
supports window logs 10..=21 and independent hash/chain logs 6..=18. Greedy, Lazy and Lazy2
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

Greedy, Lazy and Lazy2 select chains for resolved window logs through 14 and rows above 14.
The chain branch owns separate head/link arrays, uses chain log at most 18 and
search log at most 8, and visits at most `2^search_log` candidates. The row branch
owns separate position and byte-tag arrays of `2^hash_log` elements. It does not
allocate a chain or cap its unused log; valid search settings determine row width
and visits, capped at 64. Scalar row hashing uses the fresh-context salt on all
targets. Minimum-match settings clamp to key widths 4..=6. Target length is
validated but does not accelerate the lazy family or disable literal compression.

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
select the lazy-family storage leaves. The Fast/DoubleFast/Greedy/Lazy/Lazy2 parsers, persistent source states and
shared frame supervisor themselves forbid unsafe code and use the same checked contracts with every
backend combination.


Lazy and Lazy2 use the shared chain/row owners with depth-one and depth-two
speculation, signed checked gains and strict tie rules. Rejected probes retain
their table/frontier writes. Candidate entropy and repeats still publish only
after a complete compressed block is appended. These reference strategies are
separate from the existing named Lazy API and do not add numeric streaming,
dictionary, worker or long-distance execution.

## Bounded binary-tree requests

`ReferenceStrategy::BinaryTreeLazy2` uses the shared depth-two parser with an
independent head table and paired node edges. After known-size adjustment it
admits window log at most 21, hash and chain logs at most 19, and search log at
most 8. Chain log counts edge cells, so there are half as many nodes. Every
source width uses tree storage; no row table is selected. Minimum-match 3/4 and
6/7 equivalences, ignored target length, known-source hints, transactional repeat
publication and fallible exact-allocator ownership follow the qualified contract.
The `unsafe-reference-tree` feature selects checked tree-cell access independently
of every other backend. The bounded Optimal and Ultra parsers below have separate
resource limits, including Ultra2 below.

Ordinary sequence/state conformance and public cross-decoding are documented in
[tree validation](reference-tree-validation.md). Rust entropy choices remain
independent; C-identical compressed frames and compression ratios are not promised.


## Bounded Optimal requests

`ReferenceStrategy::Optimal` implements the pinned no-dictionary optLevel0
parser for complete sources of 64..=32,768 bytes contained in their windows.
Admission follows known-size resolution: window log 10..=16, hash/chain logs
6..=12, search log 1..=6, minimum-match setting 3..=7 and target 0..=4095.
Default levels 11/12 fit these caps for sources of 64..=2,048 bytes. Larger
admitted sources require bounded explicit table/search settings. The ordinary
zero-override semantics still apply; zero does not force a zero preset value.

Seven independent parser/search owners hold frequency counts, fixed path and
candidate cells, tree heads/edges and optional three-byte-key heads. Literal
and sequence scratch retain two additional allocator identities. Minimum 3
uses the short-key table and permits three-byte matches; other settings use
four-byte minimum matches. Exact wire repeat choices survive token collection.
The shared frame entropy path publishes repeats only for compressed output.
Argument/resource rejection occurs before cloning or allocating; subsequent
allocation errors release every temporary owner. Input/source hints, frame flags
and independent entropy behavior follow the existing complete-buffer API.

Sources through 16,384 bytes preserve the existing single-block path. Sources
of 16,385..=32,768 bytes use three or four balanced blocks, each at most 8,192
bytes. Search and frequency history survive raw/RLE fallback; repeat offsets
and entropy commit only for emitted compressed blocks. The output reservation
includes every block header. The whole source must fit the resolved window, so
this does not introduce window expiry, dictionary/prefix support or numeric
streaming. Tiny inputs and sources above 32 KiB remain outside Optimal's scope.
Ultra and Ultra2 retain their separate single-block limits below.

```rust
use zstdoxide::{CompressionParameterRequest, ReferenceStrategy,
    compress_with_parameters, decompress};
let input: Vec<u8> = b"retained Optimal history across bounded blocks"
    .iter().copied().cycle().take(32_768).collect();
let mut request = CompressionParameterRequest::with_level(1);
request.set_strategy(Some(ReferenceStrategy::Optimal));
request.set_window_log(16)?;
request.set_hash_log(6)?;
request.set_chain_log(6)?;
request.set_search_log(1)?;
request.set_min_match(3)?;
let frame = compress_with_parameters(&input, &request)?;
assert_eq!(decompress(&frame, input.len())?, input);
# Ok::<(), zstdoxide::Error>(())
```

The retained implementation and its independent wire-history/native checks are
described in [retained Optimal validation](reference-optimal-retained-frames-validation.md).
The final Module 20 full feature regression and complete-frame measurements
remain separate work; the earlier fresh Optimal benchmark is not a retained
performance result.


## Bounded Ultra requests

`ReferenceStrategy::Ultra` implements the pinned no-dictionary optLevel2
policy for one complete source of 64..=16,384 bytes contained in its window.
It has the same resolved geometry limits and nine scratch owners as Optimal.
Default levels 13/14 fit for 64..=2,048 bytes. Its fractional prices, frequency
lifecycle, thorough path relaxation and match-plus-literal alternatives are
qualified together. The public entry point does not expose scripted matches
or retained parser state.

```rust
use zstdoxide::{compress_at_level, decompress};
let input = vec![b'x'; 512];
let frame = compress_at_level(&input, 13)?;
assert_eq!(decompress(&frame, input.len())?, input);
# Ok::<(), zstdoxide::Error>(())
```

Explicit larger sources require bounded parameters after known-size resolution:

```rust
use zstdoxide::{compress_with_parameters, decompress,
    CompressionParameterRequest, ReferenceStrategy};
let input: Vec<u8> = (0..4096).map(|i| (i % 31) as u8).collect();
let mut request = CompressionParameterRequest::with_level(13);
request.set_strategy(Some(ReferenceStrategy::Ultra));
request.set_window_log(16)?;
request.set_hash_log(12)?;
request.set_chain_log(12)?;
request.set_search_log(6)?;
request.set_min_match(3)?;
request.set_target_length(128)?;
let frame = compress_with_parameters(&input, &request)?;
assert_eq!(decompress(&frame, input.len())?, input);
# Ok::<(), zstdoxide::Error>(())
```

Valid Ultra requests outside these local bounds return `ResourceLimit` before
allocator cloning. Complete-buffer Ultra does not admit retained
blocks, dictionaries, window expiry, numeric streaming, long-distance matching
or workers. Safe and opt-in unsafe storage backends share checked admission,
allocator ownership and output publication contracts.


## Bounded Ultra2 requests

`ReferenceStrategy::Ultra2` uses the same bounded optLevel2 parser geometry
and nine scratch owners as Ultra. It first parses the complete source to seed
frequencies, discards all first-pass tokens, invalidates search history with
checked coordinate rebasing, and reparses from the original 1/4/8 repeat offsets.
Only the second pass supplies tokens to the shared entropy encoder. The phase
transition and whole-owner reset allocate no additional parser/search storage.

Default levels 16/17 admit sources of 64..=2,048 bytes. Explicit settings admit
64..=16,384 bytes contained in the window, with resolved window log 10..=16,
hash/chain logs 6..=12, search log 1..=6, minimum match 3..=7 and target 0..=4095.
Automatic literal prices and the initial pass are always enabled publicly.

```rust
use zstdoxide::{compress_at_level, decompress};
let input = vec![b'x'; 512];
let frame = compress_at_level(&input, 16)?;
assert_eq!(decompress(&frame, input.len())?, input);
# Ok::<(), zstdoxide::Error>(())
```

```rust
use zstdoxide::{compress_with_parameters, decompress,
    CompressionParameterRequest, ReferenceStrategy};
let input: Vec<u8> = (0..4096).map(|i| (i % 31) as u8).collect();
let mut request = CompressionParameterRequest::with_level(16);
request.set_strategy(Some(ReferenceStrategy::Ultra2));
request.set_window_log(16)?;
request.set_hash_log(12)?;
request.set_chain_log(12)?;
request.set_search_log(6)?;
request.set_min_match(3)?;
request.set_target_length(128)?;
let frame = compress_with_parameters(&input, &request)?;
assert_eq!(decompress(&frame, input.len())?, input);
# Ok::<(), zstdoxide::Error>(())
```

Out-of-scope resolved Ultra2 resources return `ResourceLimit` before allocator
cloning. Tiny sources, retained public blocks, dictionary/prefix state, window
expiry, numeric streaming, long-distance matching and workers remain excluded.
The borrowed input and request need not outlive the returned owned frame.
