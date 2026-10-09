# Ordinary Fast frame publication observations

This collection freezes unchanged facebook/zstd v1.5.7 normal frame-mode behavior
at revision `f8745da6ff1ad1e7bab384bd1f9d742439278e99`. It is a test oracle, not a
production C dependency and not a Rust Fast implementation. The upstream code is
licensed under BSD-3-Clause or GPL-2.0; see the repository's `LICENSE.zstd` for
the selected upstream license. The helper's original bytes are preserved.

The bounded gzip JSON asset contains 23 fixed source-recipe observations and
11 separately labeled partial-header captures. A contiguous frames.bin pack holds
23 complete scheduled frames and four additional aligned whole-buffer frames;
checked offsets, lengths and individual hashes cover every byte exactly once.
Generated cases.rs exposes these bytes and source-recipe identifiers to Cargo
tests without requiring gzip/JSON crates. Source bytes regenerate from the
fixed xorshift and copy recipes in the pure Python support module. No recipe was
changed to produce an expected outcome after the first C capture.

The main campaign covers 1,222,674 source bytes and 64 nonempty blocks: 26 raw,
two RLE and 36 compressed. Ten of twelve directed raw candidates change pending
first-two repeats yet retain committed repeats; both RLE witnesses also have
meaningfully different pending repeats. The other two raw candidates remain
recorded non-witnesses. First uniform blocks are compressed rather than RLE.
Five/six-byte inputs preserve stale sequence stores; seven bytes starts a fresh
candidate. Window17 A/B/A emits raw/raw/raw; window19 emits raw/raw/compressed.

Each capture performs 27 complete ordinary C frame decodes totaling 2,275,346
bytes, plus 11 unfinished header observations. Header-only cases use a known
pledge and zero-size Continue, then abandon the context; they do not allocate
the pledged multi-gigabyte input or claim a valid completed frame. Four/eight-byte
content-size fields switch at `UINT32_MAX`, as observed in the pinned source.

Normal capture, independent normal regeneration and an ASan/UBSan capture have
identical deterministic artifacts. The sanitizer instruments the helper and 26
unchanged upstream common/compress/decompress C translation units. Assembly and
LeakSanitizer are disabled. Original execution was native arm64 macOS with Apple
Clang 17, 64-bit pointers and 32-bit int. This is not native 32-bit evidence.

The oracle calls `ZSTD_compressBegin_advanced_internal`, ordinary frame-mode
`ZSTD_compressContinue` and `ZSTD_compressEnd`, with complete explicit Fast
parameters, known pledge, no dictionary, no external sequences, no workers/LDM,
and disabled splitting. Only the three full-block schedules and first_uniform
compare whole-buffer payloads; arbitrary flush schedules are not interchangeable.
The deprecated buffer-less entry points are intentionally selected for visible
block boundaries. `ZSTD_generateSequences` is not used: its unconditional
candidate publication is a different protocol.

Only the initial previous owner is initialized; pending state is unavailable
until a fresh candidate. Tiny inputs never expose a new candidate from stale
scratch. Normal publication swaps owners only for a compressed block. The raw
Fast third repeat cell is recorded separately from authoritative three-repeat
wire replay. No-dictionary Fast does not create FSE repeat-valid state, so the
source's valid-to-check demotion is qualified and not claimed exercised. Hash
records are reconstructible final-state deltas, not chronological probe traces.

Run `python3 scripts/import_fast_frame_publication.py --check` for the default
offline check. It uses no compiler or process, caps compressed and decompressed
sizes, rejects additional gzip members/trailing bytes and duplicate JSON keys,
checks exact types/schema/counts/hash bindings, independently reconstructs source,
candidate sequences/repeats/hash buckets, raw/RLE payloads and complete block/frame
boundaries, and checks saved whole-buffer equality. It does **not** implement an
independent compressed-block decoder; full compressed decoding is recorded C
capture evidence and is rerun only during explicit regeneration.

For explicit immutable regeneration:

```
python3 scripts/import_fast_frame_publication.py --upstream /tmp/zstdoxize-upstream-1.5.7 --output /tmp/zstdoxide-fast-frame-replay --sanitize
```

The output must be fresh and external to the repository, collection and pinned
checkout. Regeneration verifies clean pinned source and actual header dependencies,
records compiler/commands/environment/logs/hashes, runs normal and optionally all
used C sanitizer builds, and compares complete frame bytes and logical records.
Only the reported host pointer width is excluded from cross-host semantic equality;
32-bit C int remains required. Saved fixtures are never rewritten.

The original external design, helper review, setup diagnostics, exact commands,
source hashes and native evidence are preserved by the coordinator. Initial setup
failures were a raw-blob comparison of an unrelated CRLF Windows package script,
an incorrect arbitrary inventory-count threshold, and strict diagnostics for the
intentionally chosen deprecated API. All were runner-only corrections before any
recipe executed; no parser, input recipe or C runtime expectation changed.
