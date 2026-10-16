# Policy-tagged frequency adapter review

The additive helper derives from the qualified Optimal v2 observer; the original
helper and every earlier packet stay unchanged. The new24-byte header adds a
checked policy word. Source copying, guard comparisons and operation offsets all
use the new offset24. The packet cap grows by exactly4bytes. All policy fields
are printed as u32 in request/operation/end records; only modes0/2 are accepted.

The first source span is checked before a local histogram is formed. Each count
is bounded by131072; scaled initial mass is at most768. Updated literal totals
are accumulated in u64 under the original4194304-byte schedule cap. The extra
fractional guard therefore cannot overflow and runs before setvbuf/output or any
upstream function. No dirty cache query is admitted. Existing sequence update
caps imply fractional sequence sums are always valid. Exact negative diagnostics
are part of the command plan, excluding unrelated guard/internal failures from
being counted as malformed-input successes.

All source/file ownership and canary protections from the preceding observer
remain. File opening additionally uses no-follow/nonblocking flags and fstat of
the actual descriptor; Darwin extensions make the flags visible on macOS. The
helper has no caller-controlled pointers or heap allocations. Local histogram
storage fits the existing conservative64KiB stack allowance. The source guard
and original-packet comparison detect writes, not logical overreads. All26codec
units are instrumented for ASan/UBSan and debug assertions are retained.

The only policy-dependent native calls changed from optLevel0 are rescale,
base-price refresh, raw literal price, LL price and match price. The original
update function is policy-independent and unchanged. Each batch calls it once
per primitive update; no observer-side update shortcut replaces native work.
Whole-mode independent replay already reconstructs every byte of the original
native fixture. Full normal/instrumented successor capture and independent
forged-evidence audits remain required before Rust lifecycle changes.
