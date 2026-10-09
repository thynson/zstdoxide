# Inspecting reference compression parameters

The parameter API describes facebook/zstd v1.5.7 settings without allocating
codec storage or changing an encoder. Reference strategies are separate from
the supported `CompressionStrategy` policies. A valid reference profile does
not imply that the Rust encoder implements its algorithm or can allocate its
search tables. Numeric live compression remains a later milestone.

## Validating concrete values

`RawCompressionParameters` accepts seven unvalidated numeric fields. Validation
checks each field's concrete bounds and returns a record with private fields.
It neither clamps values nor chooses automatic settings. A zero target length
is valid; the other zero fields in the raw default record are invalid.

```rust
use zstdoxide::{Error, RawCompressionParameters, ReferenceStrategy};

let raw = RawCompressionParameters {
    window_log: 17,
    chain_log: 16,
    hash_log: 17,
    search_log: 1,
    min_match: 5,
    target_length: 0,
    strategy: 2,
};
let checked = raw.validate()?;
assert_eq!(checked.to_raw(), raw);
assert_eq!(checked.strategy(), ReferenceStrategy::DoubleFast);
assert_eq!(checked.hash_log(), 17);

let invalid = RawCompressionParameters { hash_log: u32::MAX, ..raw };
assert_eq!(invalid.validate(), Err(Error::InvalidParameter));
assert_eq!(ReferenceStrategy::try_from(0), Err(Error::InvalidParameter));
# Ok::<(), zstdoxide::Error>(())
```

Bounds follow the target pointer width: maximum window/chain logs are 30/29 on
32-bit targets and 31/30 on 64-bit targets. This inspection contract does not
change the independent `EncoderOptions` frame-window range. Reference strategy
codes are converted with `TryFrom<u32>`; enum discriminants are not C codes.

## Keeping requested values

`CompressionParameterRequest` retains a normalized level, optional overrides
and an optional source hint. Level zero means three; levels clamp to
`-131072..=22`. Changing the level preserves explicit overrides. Zero clears a
numeric override to automatic, while `set_strategy(None)` clears the strategy.
Getters report requests rather than effective values chosen for a source.

```rust
use zstdoxide::{CompressionParameterRequest, Error, SourceSize};

let mut request = CompressionParameterRequest::with_level(3);
request.set_hash_log(14)?;
request.set_level(9);
assert_eq!(request.level(), 9);
assert_eq!(request.hash_log(), Some(14));

let before = request;
assert_eq!(request.set_window_log(9), Err(Error::InvalidParameter));
assert_eq!(request, before);
request.set_hash_log(0)?;
assert_eq!(request.hash_log(), None);
request.set_source_hint(16_384)?;
assert_eq!(request.source_hint(), Some(16_384));

assert_ne!(SourceSize::Known(0), SourceSize::Unknown);
# Ok::<(), zstdoxide::Error>(())
```

A source hint is not a pledge. Its range is `1..=i32::MAX`, with zero disabling
it; invalid setters leave the entire request unchanged. `SourceSize::Known(0)`
means an empty input, whereas the C public zero-size argument means unknown.
The reference reserves `u64::MAX` for unknown-size inspection.

`ReferenceFrameParameters::default()` enables content-size and dictionary-ID
emission and disables checksum, matching the reference query defaults. Rust's
live encoder continues enabling checksum by default. See
[streaming configuration](streaming-usage.md) for supported live settings and
[the parameter design](compression-parameters-design.md) for the implementation
and compatibility boundaries.
