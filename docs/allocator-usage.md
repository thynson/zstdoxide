# Using explicit allocation ownership

The supported compiler for these APIs is the exact dated toolchain recorded in
`rust-toolchain.toml`. The implementation uses the standard `Allocator` and
`Vec<T, A>` API without crate feature gates. Rust 1.100 and stable 1.101 have not
been verified; see [compiler evidence](allocator-validation.md).

Convenience APIs retain Global defaults. Functions ending in `_in` accept the
allocator as their last argument. A returned vector or dictionary retains its
allocator for its complete lifetime. Contexts own their workspace allocations;
input and output slices passed to `process` are borrowed only for that call.

## Returned output and dictionary storage

Pass an allocator value to transfer its ownership into the result. Aggregate
operations need `Allocator + Clone` to construct separate workspace owners.
Clones need not represent interchangeable allocation domains: every collection
retains the exact value that allocated its storage. No allocation is transferred
between allocator instances or converted through raw parts.

Passing `&allocator` uses the standard reference implementation. The reference
is Clone even when the allocator itself is not. All returned owners then carry
that reference and must be dropped before the allocator can be dropped.

```rust
extern crate alloc;
use alloc::alloc::Global;
use zstdoxide::{compress_in, decompress_in};

let allocator = Global;
let source = b"explicit output ownership, explicit output ownership";
let frame = compress_in(source, &allocator)?;
let output = decompress_in(&frame, source.len(), &allocator)?;
assert_eq!(output.as_slice(), source);
# Ok::<(), zstdoxide::Error>(())
```

Global is used here to keep examples independent of an application allocator.
The same signatures accept a conforming arena, counting or other allocator.
An allocator's own Clone implementation may allocate or panic; those effects
are outside the codec's fallible collection-allocation guarantee. A reference
avoids cloning the underlying allocator value.

`Dictionary::raw_in`, `from_bytes_in` and `from_bytes_with_mode_in` own their
serialized bytes. Formatted dictionaries also own their validated entropy
tables. `train_raw_dictionary_in`, `train_dictionary_in` and
`finalize_dictionary_in` route selection and validation scratch through fresh
allocator owners; returned dictionaries retain no borrow of samples/content.

```rust
extern crate alloc;
use alloc::alloc::Global;
use zstdoxide::{
    Dictionary, DecodeOptions, compress_with_dictionary_in,
    decompress_with_dictionary_in,
};

let allocator = Global;
let dictionary = Dictionary::raw_in(b"common field names and repeated values", &allocator)?;
let source = b"common field names and repeated values: repeated values";
let view = dictionary.view();
let frame = compress_with_dictionary_in(source, &view, &allocator)?;
let output = decompress_with_dictionary_in(
    &frame, &view, DecodeOptions::new(source.len()), &allocator,
)?;
assert_eq!(output.as_slice(), source);
drop(dictionary); // Results retain only their own allocations, not the dictionary.
assert_eq!(output.as_slice(), source);
# Ok::<(), zstdoxide::Error>(())
```

## Borrowed and shared context attachments

`Encoder<A, D>` and `Decoder<A, D>` use a sealed `DictionaryHandle` contract.
`D` can be an immutable `DictionaryView<'d>` or a standard shared dictionary
owner. Dictionary and workspace allocator types may differ. Prepared decoding
tables are borrowed during a call; encoding builds its own inverse tables when
needed. Contexts never store references into their own adjacent fields.

To keep dictionary construction and every codec payload allocation within a
chosen allocator domain, construct the dictionary in that allocator and attach
its view. Standard `SharedDictionary<A>` uses Global for the reference counts
and entire inline `Dictionary<A>` object; only its byte/table payloads use A.
The borrowed alternative creates no such outer allocation.

```rust
extern crate alloc;
use alloc::alloc::Global;
use zstdoxide::{Dictionary, Encoder, EncoderOptions, Decoder, DecodeOptions};

let dictionary_allocator = Global;
let workspace_allocator = Global;
let dictionary = Dictionary::raw_in(b"application-owned dictionary", &dictionary_allocator)?;
let view = dictionary.view();
let mut encoder = Encoder::with_dictionary_in(
    EncoderOptions::default(), view, &workspace_allocator,
)?;
let mut decoder = Decoder::with_dictionary_in(
    DecodeOptions::new(4096), view, &workspace_allocator,
);
encoder.reset();
decoder.reset();
encoder.set_prefix(view)?;
decoder.set_prefix(view)?;
drop((encoder, decoder));
drop(dictionary);
# Ok::<(), zstdoxide::Error>(())
```

A view's owner must remain alive while a context can use it. The compiler rejects
an escaping decoder:

```compile_fail,E0597
use std::alloc::Global;
use zstdoxide::{Dictionary, Decoder, DecodeOptions};
let decoder = {
    let dictionary = Dictionary::raw(b"short-lived content").unwrap();
    Decoder::with_dictionary_in(DecodeOptions::new(128), dictionary.view(), Global)
};
drop(decoder);
```

The same requirement applies to an encoder's retained prefix:

```compile_fail,E0597
use std::alloc::Global;
use zstdoxide::{Dictionary, Encoder, EncoderOptions};
let encoder = {
    let dictionary = Dictionary::raw(b"short-lived content").unwrap();
    let view = dictionary.view();
    let mut encoder = Encoder::with_dictionary_in(EncoderOptions::default(), view, Global).unwrap();
    encoder.set_dictionary(None).unwrap();
    encoder.set_prefix(view).unwrap();
    encoder
};
drop(encoder);
```

The context's handle type fixes its borrowed lifetime. Clearing an attachment
does not promise to shorten that type's lifetime; create another context when
switching ownership representations. Persistent dictionaries survive reset.
Unclaimed prefixes survive reset, while a claimed prefix is released after its
frame or reset. Skippable frames do not claim decoder prefixes.

## Failure, reset and measurements

All codec payload growth uses fallible reservation. A failed payload request returns
`Error::AllocationFailed`; whole-buffer helpers drop partial output and scratch.
`Dictionary::into_shared` separately uses the standard infallible Global Arc/Rc
outer allocation, including counts and the inline dictionary object.
A streaming process error poisons the context until reset. Input consumed or
output touched before failure is not rolled back, and a frame's output remains
provisional until its checksum has been validated. Encoding commits candidate
entropy only after accepting the corresponding emitted block.

Reset preserves reusable owners and their allocation identities. Changing a
decoder's window cap replaces its ring owner. It does not imply an automatic
high-water shrinking policy. An allocator reference and a borrowed dictionary
must remain valid through reset and final context destruction.

`retained_capacity()` reports reachable heap capacity, excluding inline state
and caller buffers. It includes an attached dictionary's payload once even if
another owner/context also reaches it, and excludes shared outer storage.
It is therefore not a whole-object footprint, an allocation peak, or exclusive
ownership accounting. The test allocator separately records consistent layout
sizes, requests, failed requests, live/peak payloads and final release. Its own
ledger and mutex bookkeeping are outside those payload measurements.

Allocator control does not provide static workspace initialization, C-compatible
footprint estimators, stable output-buffer semantics, or automatic shrinking.
Their upstream assertions remain tracked separately in
[the allocation test plan](upstream-allocation-plan.md).
