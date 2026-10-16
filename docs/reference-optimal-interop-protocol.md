# Bounded Optimal public-frame interoperability

This additive campaign preserves the 1,197 previous tree public source/request
recipes and adds complete-fresh Optimal requests. The initial planned domain is
source 64..16,384 within its resolved window, W10..16/H6..12/C6..12/S1..6,
minimum 3..7 and target <=4095. Original selector and numeric assertion inputs
remain unchanged. Strategy-7 requests outside this scope return resource errors;
strategies 8/9 remain unsupported. This document does not enable the public gate.

The campaign reuses immutable ZGOI0001 request / ZGOR0001 and ZGOC0001 response
helpers. The pinned C helper applies ordinary public parameters and emits complete
frames; it reports no applied scalar tuple. New tuples are reconstructed from
pinned presets and both known-source adjustments, then compared with observed
Rust resolved values. No C/Rust entropy-byte identity is expected. Safe/all Rust
frames must match; normal/full-codec sanitized C frames must match. Both codecs
decode the other's complete frame to the exact original source.

New inputs include independently observed fresh sources, default levels 11/12
at admitted sizes, explicit minimum/target/geometry bounds, equal-resolved-tuple
aliases, and resource/unsupported cases. Every hypothesis, source/request hash,
classification and tuple is frozen before public native execution. Existing
malformed packet controls remain applicable to all four helpers. Source and
frame caps retain the 4-MiB / 5-MiB predecessor transport bounds, although new
Optimal positives are at most 16 KiB. No timing or general streaming claim follows.
