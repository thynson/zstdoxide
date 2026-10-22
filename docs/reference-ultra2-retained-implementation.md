# Bounded retained Ultra2 implementation qualification

## Dedicated constructor capability

The dedicated `Scope::Ultra2Retained` constructor consumes the existing three
parser owners for policy 2 and physical source lengths 14..32768. It resets them
without changing allocator identity. The configuration accessor rejects used,
poisoned or initialized-frequency state. Every previous parser/provider entry
rejects this scope before observation or search mutation. It does not yet admit
retained Ultra2 execution or change public strategy-9 responses.

Eight focused tests (four existing retained Ultra and four new retained Ultra2)
pass in all 22 products: safe, nineteen isolated unsafe features, all features
and release/all features. The same nine public predecessor integration tests
pass separately in safe and all-feature builds. All 21 true no_std products,
formatting and strict all-target/all-feature Clippy pass. The evidence includes
changed Rust sources and complete command output. Unchanged predecessor fixture
payloads are pinned to exact Git blobs and SHA-256 digests, with a checked restore
script in the archive; full snapshots also remain in the external stage. Rust test executables are
identified by hashes and remain in the recorded target directory, with their
contents explicitly excluded from this archive. This is a focused capability
qualification, not a replacement for the final complete regression matrix.

Evidence: `docs/evidence/2026-10-22-ultra2-retained-constructors-compact.{json,tar.gz}`.
Archive SHA-256: `a0467d552fc894d6b63842cf5a54ab6214f4872e2824b106b6a220d272f8af9e`.
