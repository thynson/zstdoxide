# Reference admission policy, version 4

This immutable source-only successor records bounded Lazy/Lazy2 admission before
public API changes. All 81 original selectors, 43 numeric-loop inputs, 92 original
interop requests, 159 broader DoubleFast requests and 422 Greedy-era public
requests retain their source and request bytes. Predecessor fixtures and observed
outcomes remain unchanged. No codec, compiler or child process executes here.

| Corpus | Version 3 success / unsupported / other | Version 4 |
| --- | --- | --- |
| 81 selectors | 62 / 15 / 4 advanced | 66 / 11 / 4 advanced |
| 43 numeric loops | 10 / 33 / 0 | 18 / 25 / 0 |
| 92 original public cases | 86 / 3 / 3 resource | 87 / 2 / 3 resource |
| 159 broader public cases | 121 / 33 / 5 resource | 129 / 25 / 5 resource |
| 422 Greedy-era public cases | 372 / 37 / 13 resource | 381 / 27 / 14 resource |

Five TSVs preserve prior/successor statuses together. Four newly admitted
selectors are `unsupported-58` through `unsupported-61`. The original empty and
initial-match numeric loops add levels 5 through 8. Preset fields are original C
values before known-source adjustment. Historical interop scalar tuples are
Rust resolution observations from unchanged completed captures; they are not
new C applied-parameter observations. The Greedy archive is read sequentially
with bounded members/chunks and no extraction or archived code execution.

Lazy and Lazy2 share Greedy's window-log cap 21 and hash-log cap 19. Windows at
most 14 select chains with chain log at most 18 and search log at most 8; larger
windows select rows, ignore the unused chain dimension and cap visits at 64.
Unsupported strategies still reject before resource limits. Fast and DoubleFast
caps are unchanged. These are local execution limits after known-size adjustment,
not upstream scalar validity limits. A formerly unsupported high-resource Lazy
request becomes resource-limited rather than admitted.

Each newly successful historical public request requires additive C execution.
Old rejected records remain unexecuted. This mapping alone does not complete
public interoperability, any original source-unit migration, streaming contexts,
dictionary/worker modes or broader codec replacement obligations.

    python3 scripts/check_reference_admission_v4.py --check
