# Module 12 bounded Fast platform validation

The frozen production snapshot is commit `cc09cc90a7f4c1cd38b10aeb0371671abb6f0ed1`. All repository source files, Cargo inputs, the allocator test helper, saved test fixtures and matching-feature declarations are hashed in `source-manifest.json`; source identity was checked before and after every command. Current production/dependency files were compared with that snapshot at completion. No repository files were edited for this work.

| Target | Backend | Existing parser tests | Temporary public tests | Execution |
| --- | --- | ---: | ---: | --- |
| i686-unknown-linux-gnu | Safe/default | 3 | 3 | Strict-provenance Miri, 32-bit little-endian |
| i686-unknown-linux-gnu | All 12 unsafe features | 3 | 3 | Strict-provenance Miri, 32-bit little-endian |
| s390x-unknown-linux-gnu | Safe/default | 3 | 3 | Strict-provenance Miri, 64-bit big-endian |
| s390x-unknown-linux-gnu | All 12 unsafe features | 3 | 3 | Strict-provenance Miri, 64-bit big-endian |

All **24 interpreted executions** passed, with exact selector names and nonzero counts required. The existing parser tests cover distance-one overlap and lengths 7/8/9/12/16/64, seven-byte cleanup, invalid ranges/windows/steps and usize overflow boundaries, and each of the two parser output-reservation failures followed by successful retry and balanced exact-owner release. They do not execute the full 142-case direct corpus.

The three temporary functions exist only in the external snapshot, not in repository test suites. They cover both public APIs with the exact empty frame and injected output allocation failure; borrowed non-Clone owners on 6/7/16/64-byte sources with matching bytes, headers, source-size windows and full decoder roundtrips; and unsupported request/level rejection before allocator clone or allocation. They also passed a three-test native host smoke command before interpretation. This is the only native runtime execution in this record.

Four thumbv6m checks passed: safe and all-12-unsafe-feature `no_std` library compilation, and safe/all caller rlib code generation. The caller concretely instantiates `compress_at_level_in` and `compress_with_parameters_in` with a borrowed Global allocator. A separate exact Rc type equality checks the existing dictionary fallback on a target without `target_has_atomic="ptr"`; it is not a new Fast dictionary capability. No target runtime, final firmware link, hardware allocator, hardware memory-fit or performance result is claimed.

The dated compiler is `nightly-2026-10-08`, rustc 1.101.0-nightly (1d81eb4ad, 2026-10-07). Both cached interpreted sysroots are fully hashed. Miri uses `-Zmiri-strict-provenance`. The temporary harnesses unnecessarily enable the older `allocator_api` feature name, and the compiler emits a partial-stabilization warning. Those diagnostics are retained; production files were not changed, and this report does not claim warning-free temporary code or stable-toolchain support.

There were 19 commands: 6 compiler/target/lock metadata commands, 1 native temporary smoke command, 8 Miri commands and 4 thumb library/caller commands. All returned zero; no failures, ignored tests or task-owned processes remain. Each command, effective explicit environment, duration, log hash and actual selected test name is recorded in `evidence.json`. Durations are validation timings, not benchmarks.

The archive excludes executables, compiler build artifacts, copied reference source and the repository snapshot. It includes the complete snapshot hash catalog, both temporary harnesses, their Cargo/lock configuration, exact runner, report and all logs. The repository snapshot can be reconstructed from the recorded commit and archive paths, with only the explicitly hashed temporary test added.
