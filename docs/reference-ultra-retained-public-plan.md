# Retained Ultra public hypotheses

The additive source-only plan covers 1593 requests: the complete 1513-case
retained Optimal predecessor and 80 independent additions. It predicts 1423
successes and 170 resource rejections. The additions contain 61 successes and
19 resource rejections. No current public admission or native execution is
established by these hypotheses.

Exactly one predecessor classification changes: index 1511,
`retained-resource-ultra`, with 32768 uniform bytes and request
`[11,16,12,12,6,3,4095,8,0]`, resolves to W15/C12/H12/S6/minimum3/target4095/strategy8
and is proposed to move from resource rejection to success. Its source hash is
`b217b65e6f205f41b3fb8ef90cf7c44da93f630ca03965273485bbb21a5cccf5`.
All other predecessor source/request/resolved-tuple/classification values remain
identical to the pinned predecessor plan. Their actual complete response bytes
must still be checked during public execution.

New recipes cover source/block-count boundaries 16385, 24576, 24577 and 32768,
independent uniform/periodic/noise sources, default-level resolution, minimums
3..7, targets 0/1/4095, retained geometry endpoints and source/window/resource
rejections. The resolved source must fit its window; window logs stay at most16,
hash/chain logs at most12, search logs at most6 and targets at most4095. Explicit
W14 requests at 16385 and 32768 remain resource errors. Retained Ultra2 remains
outside admission. Default levels retain their independently resolved strategy
and resource limits; the plan does not force a strategy.

Both ordinary and optimized Python generate exactly the same manifest and pass
15 malformed-input controls plus 33 positive boundary/geometry controls. Child
execution is blocked. All source inputs and every upstream admission inventory
row remain pinned. Strategy-8 tuples are independently reconstructed by the
positive-level resolver; other predecessor families retain their own already
qualified pinned resolution, including negative-level Fast requests.

Three historical source-only failures are preserved: an omitted pinned protocol
document in the first snapshot, applying the positive-level resolver to inherited
negative levels in the second attempt, and a geometry-loop binding shadowing the
preset table in the third. Additive helpers/configurations correct these issues;
none of the failed scripts or captured inputs is edited or relabeled successful.
Their complete source manifests reconstruct byte-for-byte from the successful
successor snapshot, as checked by the separate reconciliation receipt.

The frozen manifest is
`tests/fixtures/reference-retained-ultra-public/recipes.json`, SHA-256
`510db8c9072d9f64f9919f8863b7b60bd1c2fd28e8c096172dc5dd1d2fb599db`.
The complete five-command receipt, earlier failures, source snapshots, generated
manifests and reconstruction records are preserved in
`evidence/2026-10-21-ultra-retained-public-recipes.{json,tar.gz}`.
Archive SHA-256: `cf68b772e471aa98de3cb720dbcfdc23fb7d194e36f0819a71ea2f92d35c4823`.

Public qualification must preserve 3024 response byte strings for the 1512
unchanged predecessors and verify the two safe/all responses for the declared
transition separately. The 61 new successes plus that transition require pinned
normal/sanitized native encoding and exact bidirectional decoding. Safe/all Rust
frames must agree; Rust/native entropy choices need not agree. Full regressions,
foreign platform binding and benchmarks remain subsequent gates.
