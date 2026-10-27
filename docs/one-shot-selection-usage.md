# Reusable one-shot dictionary selection

`OneShotDecoder` retains dictionary selection and copied limits across calls.
Input, output, frame/entropy state and scratch belong to each call. There is no
promise of workspace reuse or native context reset/parameter equivalence.

```rust
use zstdoxide::{DecodeOptions, Dictionary, OneShotDecoder};
let dictionary = Dictionary::raw(b"raw dictionary bytes")?.into_shared();
let mut decoder = OneShotDecoder::with_dictionary(
    DecodeOptions::new(1024 * 1024), dictionary,
);
// Each call can decode an exact concatenation into independent output storage:
// let written = decoder.decompress_into(encoded, output)?;
# Ok::<(), zstdoxide::Error>(())
```

`with_dictionary`/`set_dictionary(Some(handle))` retain persistent selection until
replacement. `with_prefix`/`set_prefix(Some(handle))` install selection for exactly
the next attempted call, covering every standard frame in that call. A prefix is
removed before preflight/allocation and drops when the attempt returns or unwinds.
It is consumed even for empty input, skips only, malformed bytes, output/window
limits, allocator failure or invalid in-place coordinates. Persistent selection
survives those errors. This differs from incremental `SliceDecoder`/`Decoder`
prefixes, which are frame scoped.

Both setters replace the entire selection; None clears it. `dictionary()` borrows
its immutable view, and `prefix_pending()` identifies next-call ownership. Options
can be inspected or replaced without modifying selection. No handle clone is
required. Use `without_dictionary` when generic handle inference is supplied by a
later borrowed view, or `new` for the default SharedDictionary owner.

The `_in` methods accept an independent scratch allocator per call. They delegate
existing allocator cloning and failure contracts, retaining no allocator. Borrowed
dictionary views must outlive the coordinator selection. Shared-owner allocation
and caller-side cloning retain ordinary Rust semantics.

`decompress_in_place` decodes a suffix of an exclusive buffer with the layout,
provisional-output and successful-tail contracts documented in
[in-place output usage](in-place-output-usage.md). Dictionaries remain independent
of that buffer. `decompress_into` requires independent input/output borrows.
Success reports the number of bytes written; errors may leave provisional output
inside the admitted range. A fresh call always starts fresh frame/history state.
Native empty-input acceptance remains an explicit difference: Rust returns
UnexpectedEof, after consuming any pending prefix.
