# Configuring named match search

`EncoderOptions` selects the Greedy, Chain or Lazy policy and frame settings.
`MatchSearchOptions` controls those policies' actual search storage and effort.
These are independent Rust policies; reference compression-level inspection does
not select or configure them. Whole-buffer compression helpers keep the defaults.

| Search setting | Default | Accepted range |
| --- | --- | --- |
| Hash log | 16 | 10–18 |
| Working chain-node log | 18 | 10–19 |
| Key bytes | 4 | 4–8 |
| Chain visits per probe | 8 | 1–64 |
| Lazy visits per probe | 16 | 1–64 |
| Retained output-history bytes | 131,072 | 1–1,048,576 |

Zero is invalid for every setting. Setters validate before changing the record.
Greedy always examines one candidate. Chain and Lazy share chain storage, but
use separate visit limits; Lazy applies its limit to both current and next-byte
probes. A rejected or stale candidate still consumes a visit. Wider keys do not
change the four-byte minimum emitted match length. Output history is further
limited by the advertised frame window; blocks remain at most 128 KiB. Earlier
bytes within the current block and dictionary matches have separate eligibility
rules, so this history limit does not cap every emitted match distance.

## Installing search settings

Install settings before the first `process` call or after session reset. Even an
empty input/output process call starts the header and closes this boundary.
Frame-option replacement preserves search settings, and search replacement
preserves frame settings, dictionary/prefix selection, counters and allocations.

```rust
use zstdoxide::{CompressionStrategy, Encoder, EncoderOptions, EndDirective,
               Error, MatchSearchOptions, StreamStatus, decompress};

let mut encoder = Encoder::new();
assert_eq!(encoder.options().strategy, CompressionStrategy::Greedy);
assert!(encoder.options().checksum);
assert_eq!(encoder.search_options(), MatchSearchOptions::default());

let mut search = MatchSearchOptions::new();
search.set_hash_log(14)?;
search.set_chain_log(15)?;
search.set_key_bytes(6)?;
search.set_lazy_visits(24)?;
search.set_history_bytes(256 * 1024)?;
let before = search;
assert_eq!(search.set_key_bytes(3), Err(Error::InvalidParameter));
assert_eq!(search, before);

encoder.set_options(EncoderOptions {
    strategy: CompressionStrategy::Lazy,
    window_log: 18,
    ..EncoderOptions::default()
})?;
encoder.set_search_options(search)?;
assert_eq!(encoder.search_options(), search);
assert_eq!(encoder.search_retained_capacity(), 0); // Configuration allocates nothing.

let source = b"configured search, configured search, configured search";
let mut position = 0;
let mut encoded = Vec::new();
loop {
    let mut destination = [0; 17];
    let progress = encoder.process(
        &source[position..], &mut destination, EndDirective::Finish,
    )?;
    position += progress.consumed;
    encoded.extend_from_slice(&destination[..progress.produced]);
    if progress.status == StreamStatus::FrameFinished { break; }
}
assert_eq!(position, source.len());
assert_eq!(decompress(&encoded, source.len())?, source);
assert!(encoder.search_retained_capacity() > 0);
assert!(encoder.retained_capacity() >= encoder.search_retained_capacity());
# Ok::<(), zstdoxide::Error>(())
```

A successful configuration call neither allocates nor clones allocator or
dictionary handles. Storage is prepared when processing needs it; allocation
failure then follows the existing poisoned-until-reset processing contract.
No setting guarantees smaller frames or faster compression for every input.

## Session and parameter resets

`reset()` preserves both option records except the source pledge, which it
clears. `reset_with_options(frame)` installs validated frame settings and keeps
search settings. Both discard the session and zero its counters. Their existing
persistent-dictionary and unclaimed-prefix rules remain unchanged.

`reset_parameters()` accepts only fresh or finished sessions. It restores both
option records to Rust defaults and clears every dictionary attachment and seed,
but preserves the session phase and counters. `reset_all()` accepts every phase,
including failures, restores both defaults, clears attachments/seeds and returns
a ready session with zero counters. None of these operations reallocates caches.

```rust
use zstdoxide::{CompressionStrategy, Encoder, EncoderOptions, EndDirective,
               Error, MatchSearchOptions, StreamStatus};

let mut encoder = Encoder::with_options(EncoderOptions {
    checksum: false,
    pledged_size: Some(3),
    ..EncoderOptions::default()
})?;
let mut search = MatchSearchOptions::new();
search.set_hash_log(12)?;
encoder.set_search_options(search)?;
let mut output = [0; 32];
assert_eq!(encoder.process(b"abc", &mut output, EndDirective::Finish)?.status,
           StreamStatus::FrameFinished);
assert_eq!(encoder.set_search_options(search), Err(Error::InvalidState));

encoder.reset();
assert_eq!(encoder.search_options(), search);
assert_eq!(encoder.options().pledged_size, None);
assert!(!encoder.options().checksum);
encoder.reset_with_options(EncoderOptions {
    strategy: CompressionStrategy::Chain,
    ..EncoderOptions::default()
})?;
assert_eq!(encoder.search_options(), search);
assert_eq!(encoder.process(b"abc", &mut output, EndDirective::Finish)?.status,
           StreamStatus::FrameFinished);

let counters = (encoder.total_input(), encoder.total_output());
let retained_search = encoder.search_retained_capacity();
encoder.reset_parameters()?;
assert_eq!(encoder.options(), EncoderOptions::default());
assert_eq!(encoder.search_options(), MatchSearchOptions::default());
assert_eq!((encoder.total_input(), encoder.total_output()), counters);
assert_eq!(encoder.set_search_options(search), Err(Error::InvalidState));
encoder.reset_all();
assert_eq!((encoder.total_input(), encoder.total_output()), (0, 0));
assert_eq!(encoder.search_retained_capacity(), retained_search);
# Ok::<(), zstdoxide::Error>(())
```

## Interpreting retained capacity

`search_retained_capacity()` counts allocated capacity for both Greedy tables,
both Chain head/node pairs, and retained output history. It includes inactive
strategy caches, so returning to Greedy can retain earlier Chain storage. Smaller
settings can reuse larger allocations. Merely selecting settings does not resize
them, and reset preserves the exact allocator owners.

`retained_capacity()` additionally counts staging, pending output and other
retained codec workspaces, plus the selected dictionary's reachable payload and
prepared tables. The search-only measurement excludes that dictionary storage.
Neither measurement includes caller buffers, inline state, allocator bookkeeping
or shared outer control blocks. These are capacity observations, not requested
logical sizes, hard memory limits or process-memory measurements.

See [streaming usage](streaming-usage.md) for progress, flushing and dictionary
lifetime rules, and [allocator usage](allocator-usage.md) for allocation domains.
