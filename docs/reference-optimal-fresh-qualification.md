# Private ordinary fresh Optimal qualification

The private `parse_fresh` composition was checked on 2026-10-16 before retained
production changes. The matrix runner recorded immutable Rust/Cargo source
hashes and checked them again after all commands completed.

Each of the 19 isolated unsafe features passed 52 focused Optimal tests and a
true `--no-default-features` compilation. Complete default and all-feature suites
each passed 1,288 tests. These results preserve predecessor regressions as well
as the new ordinary startup fixture, fresh search and parser composition checks.
The focused implementation commit separately records formatting, strict Clippy,
release/all-feature checks and the six focused backend configurations.

Thirty-six strict-provenance Miri checks passed on i686 and s390x: six selected
validation, poisoned reset and exact moved/borrowed owner checks in safe,
combined search/path, and all-feature configurations on each target. This is
interpreter execution on foreign layouts, not native execution on foreign hosts.
Ten thumbv6m checks passed: the library and a concrete caller in each of five
configurations. The caller creates seven independently borrowed owners and
instantiates fresh parsing/reset code. Firmware execution was not performed.

The archives `docs/evidence/2026-10-16-optimal-fresh-{matrix,platform,thumb}.tar.gz`
contain the exact runners, source manifests and command logs. Their JSON reports
record member hashes, archive hashes and counts; every member was read back and
verified after packaging. No benchmark was run concurrently with this campaign.
Public strategy 7 remains unsupported, and retained block publication remains a
separate implementation and qualification gate.
