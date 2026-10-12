# Reference admission policy, version 3

This source-only successor freezes bounded Greedy admission before public API
changes. It preserves the original 81 selector inputs, 43 numeric-loop inputs,
92 original interop requests and 159 broader DoubleFast interop requests. The
version 1 and 2 fixtures, generators, archives and observed outcomes remain
unchanged. No codec, compiler, archived program or child process runs here.

| Corpus | Version 2: success / unsupported / other | Version 3 |
| --- | --- | --- |
| 81 selectors | 61 / 16 / 4 advanced | 62 / 15 / 4 advanced |
| 92 original interop cases | 85 / 4 / 3 resource | 86 / 3 / 3 resource |
| 159 broader interop cases | 113 / 42 / 4 resource | 121 / 33 / 5 resource |
| Empty source, levels -1 through 22 | 5 / 19 / 0 | 6 / 18 / 0 |
| Initial match, levels 1 through 19 | 3 / 16 / 0 | 4 / 15 / 0 |

Four TSVs retain predecessor and successor outcomes together. Their fixed
SHA256 identities are checked against independently reconstructed source/request
metadata and historical observations. Presets are original C values before
known-source adjustment. Interop scalar tuples are historical Rust resolution
observations, not newly observed C parameters. The broader tuples are read from
bounded exact members of the pinned chunk archive; archived code is never run.

Greedy uses window log at most 21 and hash log at most 19. Resolved windows above
14 select row storage; smaller windows select chains, with chain log at most 18
and search log at most 8. Row mode does not allocate or cap the unused chain
dimension; it bounds visits to 64 even when the valid search log is larger.
Unsupported strategies reject before operational resource limits. Fast and
DoubleFast limits remain unchanged. These are local execution limits, separate
from scalar parameter validity and known-source adjustment.

The newly admitted original-92 case and eight broader cases require additive
positive C execution. Historical rejected records remain marked not run.
`cap-w22-c19-h19-s3` changes from unsupported to resource-limited and is not
admitted. These mappings do not complete any official source-unit migration,
reusable context, mutable decoder, caller-owned output or CLI obligation.

    python3 scripts/check_reference_admission_v3.py --check
