# Reference-stream parameter lifecycle design

Module 41 design decision, before implementation. The existing
`ReferenceStreamEncoder` continues to own one private shared engine; the
compiled coordinator remains a separate fixed-profile API. No parser or table
implementation is duplicated.

## Configuration operations

Add `set_request(request)` and `set_options(options)` at a clean boundary
before header initialization. These are scalar, allocation-free changes.
Boundary validation precedes candidate profile resolution/admission; admission
uses the candidate request/options and currently pending prefix's matchable
content length. Failure leaves request, flags, prefix, counters, initialized
observation and allocations unchanged. Success stores the requested settings
without marking a frame initialized; process still resolves and validates
again before publication. Native bulk setters merely store fields, so eager
bounded-profile admission is a documented Rust restriction.

Retain the existing wrapper request copy and optional engine request. Every
constructor, setter and reset must keep them equal: request-mode-only private
engine helpers perform all fallible validation before mutation, then the
wrapper copies the same scalar record with no fallible operation between the
two writes. A helper rejects or is inaccessible to fixed compiled mode. No
public mutable engine borrow can bypass this invariant. `request()` reports pending requested settings;
`parameters()` reports the initialized current/completed frame only.

No operation permits per-field mid-frame mutation, changes the source count
already accepted, touches output slices, seeds parser tables or allocates.
The next frame can select a different admitted family and geometry.

## Reset operations

`reset()` remains session-only: discard the frame and pledge, keep the request,
other flags, unclaimed prefix and reusable workspace; release a claimed prefix.
`reset_with_options(options)` keeps its Module 40 behavior, including deferred
profile admission on the next process call. This is an unconditional session
reset, unlike clean-boundary `set_options`.

Add `reset_parameters() -> Result<()>`: accept a clean boundary or a completed
frame, restore `CompressionParameterRequest::default()` and default frame
flags, remove pending/active prefix, and retain allocations. Keep counters,
Finished phase and the completed profile observation when invoked after
completion. Reject active, finishing and poisoned sessions without mutation.
The Rust default-frame convention clears the pledge; native parameter-only
reset can keep a fresh pledge outside its requested record. Document this
bounded difference, matching existing Rust encoder reset APIs.

Add `reset_all()`: accept every phase, reset requested parameters/options to
defaults, remove pending/active prefixes, clear frame state/counters/profile
observation and retain workspace allocation owners. It performs no allocation
and recovers from processing errors. These reset methods do not compile or
attach a dictionary and do not change the borrow lifetime of the owner type.

## Ownership and invariants

Requests, options, phases and resolved parameters are plain records with the
same representation in every feature configuration. No artificial unsafe
variant is needed. Workspace `Vec` and entropy owners retain their exact `A`;
clearing requested metadata cannot call an allocator. Only a later process
call can grow workspace or instantiate the existing independently selectable
safe/unsafe parser owners for the newly resolved frame.

An uninitialized frame has no published profile; a running or completed frame's
profile cannot change. Requested fields can change at a permitted boundary
without retroactively changing bytes already emitted. Prefix views remain
borrowed, single-use metadata; request replacement retains a pending prefix,
parameter reset removes it, and session reset follows its claim state.

## Verification work items

1. Test request/options setter boundary precedence, transactional unsupported
   profile errors, initialized observation, retained pending prefixes and
   allocation-free success/error paths. Exercise family and window changes
   across reused frames with one-byte output and exact source checks.
2. Test all reset scopes at clean, active, finishing, completed and failed
   phases, with pending/claimed raw and formatted prefixes. Verify defaults,
   counters, old completed-frame observation, pledge clearing, no allocation,
   deterministic reconstruction and subsequent allocator-failure release.
3. Run guarded native bulk/reset schedules with exact requested fields and
   frame metadata, plus explicit native-only pledge-retention observations.
   Cross-decode all emitted frames. Native automatic post-frame init and
   source-pledge differences must be visible in the adapter protocol.
4. Run focused safe/all tests, full isolated-feature/all-feature tests,
   release, true no_std, strict Clippy and concrete atomic-free ARM output.
   No Miri is required unless implementation adds/modifies unsafe code.
5. Time request replacement versus fresh construction and/or session reset
   with deterministic safe/all workloads, independent repeats and output
   ledgers. Do not imply policy equivalence or universal speedup.

Live individual setters, workers, compiled-state replacement through this
wrapper, persistent loaded dictionaries, stable input/output modes and native
workspace identity remain separate future contracts.

## Public state acceptance matrix

| Operation | Clean | Active header | Finishing output | Finished | Failed |
| --- | --- | --- | --- | --- | --- |
| set_request / set_options | validate then store | reject unchanged | reject unchanged | reject until reset | reject unchanged |
| reset_parameters | defaults, no allocation | reject unchanged | reject unchanged | defaults, preserve counters/profile/Finished | reject unchanged |
| reset / reset_with_options | discard session | discard session | discard session | discard session | recover session |
| reset_all | defaults and empty session | defaults and empty session | defaults and empty session | defaults and empty session | recover with defaults |

Setters preserve an unclaimed prefix and the exact allocator owners. A failed
setter must not poison an otherwise usable frame. Reset-parameters and reset-all
remove every prefix. A failed process call still poisons the session; only a
session-reset operation recovers it. The complete adapter must distinguish
native's automatic post-frame init from Rust's explicit Finished state, and
must initialize native streaming before testing pledge mismatch so initial
Finish auto-pledging cannot erase the intended mismatch.
