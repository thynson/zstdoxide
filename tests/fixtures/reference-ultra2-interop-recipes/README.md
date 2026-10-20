# Public Ultra2 source-only hypotheses

The immutable generator `scripts/oracles/ultra2_interop.py` preserves all 1345
Ultra source/request pairs and adds 88 independently resolved cases. Its 1433
recipes contain 146881431 source bytes: 1292 admitted hypotheses and 141 resource
rejections. The 25 frame-equivalence groups include the two old explicit
strategy-9 rejections that become admitted under the new bounded policy.

All 1222 predecessor successes remain admitted with exact frame identity
obligations. Default levels 16/17 are admitted for the tested 64..2048 range;
source-size reduction does not bypass search-log limits of levels 18..22.
New cases include 26 distinct seed observations, 24 default cases, ten
minimum/target boundaries, eight explicit aliases and 20 resource controls.

Source regeneration and 15 malformed/coherent forgery controls run in normal
and optimized Python without child processes. This is not native execution,
public enabling or a cross-implementation entropy-byte identity claim.
