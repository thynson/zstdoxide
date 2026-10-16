# Optimal public interoperability audit successor

The original command-label preflight did not validate the fixed source-name
closure inherited by the auditor. An explicit check before native execution
found that its historical digest does not describe the completed Optimal
source inventory. The unchanged controller already snapshots all Rust modules
and oracle dependencies dynamically. The additive v2 auditor updates only the
source-name closure digest to `2d5c16173b3858bf94f4b2f628c9c831ac74d80770f6e8f5bf8404e5fcaaf232` (162 paths).
The original auditor/preflight, native adapter, transport, request hypotheses,
command counts and response/environment checks remain unchanged.

The v2 preflight reconstructs this exact complete inventory and rejects missing,
repeated or incomplete source-name sets. It retains all original pin, strict-JSON
and command-order controls. Removing the two recently added plan/state modules
would yield `42cc3281b74784061e7fe65917a40ee9e0767b7415e1ab17fdebc2fd1d575365`, which also differs from the stale original
auditor digest; the correction therefore covers the complete inherited inventory,
not only those modules. This is an additive inventory correction before the
controller's first native execution, without changed source/request recipes.

Normal and optimized Python preflights passed with identical command inventories,
11 rejected pin/JSON/order/source-closure controls and zero child processes.
The initial attempted two-file-only reconciliation failed and is retained with
its preparation source; the complete closure correction is explicit. Evidence:
`docs/evidence/2026-10-16-optimal-public-preflight-v2.tar.gz`.
