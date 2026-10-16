# Native-audited synthetic Ultra scalar prices

`cases.bin` contains2470scalar rows for optLevel0 and2, observed from unchanged
facebook/zstd v1.5.7 functions at revision f8745da6ff1ad1e7bab384bd1f9d742439278e99.
Normal and fully ASan/UBSan-instrumented outputs match; every result is also
independently reconstructed. See `provenance.json` for exact input, capture,
transcript, projector and fixture hashes. Derived format widths retain source
attribution through NOTICE/LICENSE.zstd.

The transport is `ZUSFIX01`, little-endian u32 count, and2470rows of ten u32 request
words plus signed i64 native result. Total118572bytes. Requests are frozen in the
separate source-only recipe manifest. Scalar synthetic states are not ordinary
CCtx lifecycle observations and do not establish complete Ultra frame support.

`scripts/check_ultra_scalar_fixture.py` reconstructs the full corpus without C
or production Rust. The borrowed Rust reader separately validates layout and
input domains before scalar implementation conformance tests consume the rows.
