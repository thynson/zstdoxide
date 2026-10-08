# Official-codec interoperability fixtures

These `.zst` files were generated with the official Zstandard v1.5.7 CLI from
original deterministic inputs. No upstream source, corpus, or copyrighted
third-party text is embedded. `generate.py` specifies every input byte and CLI
option; matching Rust generators live in `tests/interop.rs`.

The corpus covers empty and short frames, incompressible bytes, long uniform
runs, skewed literal frequencies, structured text, repeated distant data,
multiple blocks, compression levels 1/3/9/19, content checksums, and a frame
without an advertised content size or checksum. The tests compare reconstructed
bytes, truncate frame boundaries, damage checksums, enforce output limits, and
combine frames with skippable frames. These tests do not claim coverage of every
valid Zstandard stream or dictionary support.

Regenerate with the pinned executable on `PATH`:

```sh
python3 tests/fixtures/generate.py
```

Alternatively set `ZSTD_BIN` to an absolute executable path. The generator
verifies that the oracle reports v1.5.7 and round-trips the checksum-bearing
fixtures with the official decoder.

The checked-in tests require no CLI. Additional differential tests generate 56
frames at runtime using the external oracle:

```sh
cargo test --test interop -- --ignored
```

The executable is a test oracle only. The library has no C bindings or dependency
on any other codec implementation.
