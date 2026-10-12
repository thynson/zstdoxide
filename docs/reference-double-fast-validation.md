# Bounded DoubleFast validation

## Private long-key layout

The copied `SearchKeyLayout::for_double_fast_long` admits eight-byte keys at
logs 6..18 without expanding named API limits or Fast's four-through-seven-byte
key domain. Existing MatchTable owners and their independently gated access
backends serve both table roles; this scalar descriptor adds no unsafe operation.

Three focused tests pass in five configurations: safe, unsafe-match-table, all,
no-default, and no-default plus all twelve unsafe features. They check every
u8 log, exact key-length rejection, 576 existing C width-eight hash observations,
256 high-bit-derived smaller-log expectations, and independent long/short table
read/write/reset behavior including valid source index zero. The smaller-log
expectations are mathematical consequences of existing log-ten observations;
they are not new direct C captures. Dedicated DoubleFast fixtures remain open.

The frozen Fast and named baselines pass eleven tests each in safe/all, checking
unchanged complete Fast frames/allocator counts and the original named records.
An existing named/Fast domain guard also passes. All fourteen commands succeed,
including strict all-target Clippy safe/all, no_std safe/all and formatting.
The shared filter also executes an existing Fast tier test in each configuration.
There are 43 passing test executions and no ignored tests in these focused
commands; filter-based unit selections intentionally exclude unrelated cases.
See [evidence](evidence/2026-10-12-double-fast-layout.json) and the
[logs and source archive](evidence/2026-10-12-double-fast-layout.tar.gz).

This checkpoint implements no DoubleFast parser or public compression admission,
and runs no new C capture, Miri campaign or performance measurement. The copied
plan/ownership foundation, direct oracle and parser are separate work items.
