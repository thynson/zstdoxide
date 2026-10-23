# Retained Ultra2 final public platform binding

The public 16385-byte retained frame entry passes four strict-provenance Miri
executions: safe/all on i686 and s390x. Safe/all thumbv6m core-plus-alloc checks
also pass. Two concrete release probes retain LLVM IR definitions and ARM ELF32
object files for public dispatch, allocation, entropy encoding and output drop.
These are foreign interpretation and code generation, not native foreign runs.
All eight platform commands and concrete artifacts remain source/hash bound.

Evidence: `evidence/2026-10-23-ultra2-retained-public-platform.{json,tar.gz}`,
archive SHA-256 `50bd4da723c8755739db101640f2f900d51a28190b78fb630643d3f50b6c60c5`.

The final native/platform reconciliation checks 2107 final focused inputs and
one explicit historical-to-final difference in `src/reference_encode.rs`.
Only Rustdoc lines changed to describe first-block Ultra2 learning and later
retained passes accurately. Every executable Rust line remains identical;
historical native and foreign runs are not relabeled as having the corrected
documentation. The final focused campaign reruns all sixteen commands, including
formatting, strict Clippy, public/private/factory tests, examples, allocator and
safe/all true no_std checks. Full-regression doctests remain a separate gate.

The executable checker and receipt are
`evidence/2026-10-23-ultra2-retained-final-source-binding.{py,json}`.
Final focused evidence is
`evidence/2026-10-23-ultra2-retained-final-focused.{json,tar.gz}`, archive SHA-256
`3dc6abfe8bd44550b50756790ddeebfd02900b6085f0c49e7dfbb5e89f6d11a5`.
Full regression and complete-frame measurements remain distinct obligations.
