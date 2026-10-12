# Ordinary Greedy capture controller: pre-execution review

The controller `scripts/capture_greedy_frames.py` is frozen at SHA256
`19e122d3a94aa17be4628c943105fb7854d6b30259d08b87141f316b0ef4272b`.
It pins the reviewed v2 helper, both immutable source recipe sets, the independent
validator, the prior bounded process runner and their imported dependencies.
The 126 original recipes and 12 additive tiny-window-floor recipes provide
138 distinct inputs, 7,365,784 source bytes, 403 scheduled calls, 395 positive
calls, 13 whole-frame comparisons and 15 incomplete header probes.

Before creating an output directory or running a command, the controller rejects
repository/reference overlap, existing destinations and missing compilers. Path
resolution includes symlinks. The reference must remain outside the repository;
its revision and every catalogued C/header identity must match v1.5.7. Every
command checks unchanged source and input hashes before and after execution.
Only a new external directory receives generated inputs, binaries, frames and
logs. Each C helper frame destination starts absent and uses exclusive creation.

Both builds compile all 26 used common/compression/decompression C units directly.
The normal build uses O2; the instrumented build uses O1, ASan and UBSan with
recovery disabled and frame pointers retained. Assembly, legacy codecs and
workers are disabled. No support archive is linked. Upstream per-function
sanitizer annotations remain unchanged. LeakSanitizer is explicitly disabled;
there is no leak-check claim for fatal helper exits. Every command retains exact
arguments, environment, bounded stdout/stderr, exit status and hashes. A 180 s
deadline (30 s for rejection probes) and a 64 MiB per-stream cap apply. Complete
frame files must be regular, non-symlink files within the source/schedule bound
before loading. Normal/instrumented records and complete frames must agree byte
for byte. Incomplete headers are qualified separately and never decoded.

The predeclared malformed campaign has 35 commands per build: parameter and
resource limits, signed/fractional/overflowing numbers, case IDs, missing or
oversized sources, malformed or oversized schedules, block/window limits, row
parameter limits, invalid header pledges/windows and mode/arity errors. Each
must exit 1 with a helper diagnostic, no sanitizer diagnostic and no frame file.
These are bounded rejection probes, not successful ordinary frame coverage.

The retained normal and optimized Python preflight driver blocks child creation.
Both runs verify the exact plan and reject ten bad path/compiler/pin scenarios
without creating an output directory or attempting a child process. This is
pre-execution evidence only: it does not establish any C frame outcome. The next
work item must retain capture failures, validate successful observations offline,
freeze artifact identities, and qualify the observed coverage before Rust use.
