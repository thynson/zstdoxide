# Complete private Ultra2 lifecycle fixture

`cases.bin` is an immutable little-endian `ZU2FX01\0` projection of the pinned
facebook/zstd v1.5.7 private two-pass wrapper. It contains 53 requests and 394656
logical events, including complete eligibility, frequency, search, path, sequence,
repeat and reset states. `provenance.json` binds every original request and native
transcript, the exact capture envelope, case offsets and complete fixture digest.

The native observer ran normal and fully ASan/UBSan builds with two padding fills.
Every canonical state matched source-only predictions sealed before execution.
Separate normal and optimized Python models reconstruct every projected byte;
only native allocation sizes are opaque audited inputs. Both modes reject 57
fixture forgeries with the outer digest bypassed. The fixture is 49004443 bytes,
SHA-256 `1583da05247179243a25a55cfce6869ac6a13f874bfcf8cc493311f636f61b1a`.

See `docs/reference-ultra2-fixture.md`,
`docs/reference-ultra2-fixture-qualification.md`, and
`docs/reference-ultra2-native-qualification.md`. No entropy-coded block or frame
is present. Threshold 7/8/9 and opaque-prefix skip controls do not widen public
strategy admission. All predecessor fixtures remain unchanged. Upstream-derived
constants and observations retain attribution in NOTICE and LICENSE.zstd.
