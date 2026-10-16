# Fresh Optimal startup executable capture contract

This additive campaign uses the frozen twelve ZOPST01 recipes, eighteen invalid
packets and fourteen observation-only anchors from the prior source review.
The exact command inventory has revision/status/compiler observations followed
by two complete builds, normal and ASan/UBSan. Each build obtains dependency and
predefine output, compiles all 26 required translation units, links once, and
runs each packet once. Every valid helper call emits two padding passes and
creates two frames plus two allocation ledgers through exclusive file creation.

Each command has a 120-second deadline, stdout at most 128 MiB for valid runs
and 2 MiB otherwise, and stderr at most 1 MiB. The campaign is limited to 1,800
seconds and 512 MiB of combined logs. Frame files are at most 1,024 bytes and
ledgers at most 2,048 bytes. Capture roots must be disjoint from both repository
and pinned source. A fresh output root prevents accidental evidence replacement.
The compiler binary, all source inputs, all original/generated instrumentation,
commands, environment, requests, logs, objects and artifacts have retained hashes.

Only PATH/HOME/TMPDIR/DEVELOPER_DIR/SDKROOT may be inherited. LANG and LC_ALL are C,
TZ is UTC, ASAN_OPTIONS is detect_leaks=0:halt_on_error=1, and UBSAN_OPTIONS is
halt_on_error=1:print_stacktrace=1. Dependency and predefine checks happen before
helper execution. All original C/header dependencies must match the pinned
inventory; only the reviewed generated opt unit and local helper are additional.
No dynamic library injection or caller-supplied C flags are accepted.

Independent offline replay compares all 4,409 logical events per build, including
complete path/frequency/search cells, pre-query snapshots and append-only stored
sequences. Both padding passes and both build transcripts must agree. Frame
inspection verifies exact content size, one complete block, raw/RLE source and
compressed extent; it does not decode compressed payloads. The unchanged pinned
C decoder performs that check inside the helper. Separate frozen frame fixtures
and safe/all-feature Rust decoding are required before private startup admission.
Allocator audit checks bounded integer requests, live-byte high-water bounds,
complete release and equality across all four observations for each recipe.

Malformed requests must reject with one diagnostic before metadata or artifacts.
Plan/environment/dependency/frame/ledger mutations are rejected without children
in normal and optimized Python. Later capture forgeries must also reject with
child execution disabled. Failed runs are retained separately, never promoted to
successful evidence. This gate qualifies fresh candidate startup, not retained
compressed-block publication or public Optimal dispatch.
