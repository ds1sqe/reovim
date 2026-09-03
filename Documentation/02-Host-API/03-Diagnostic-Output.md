# Diagnostic Output

[Host API index](../02-Host-API.md)

## Capability boundary

**DiagnosticOutput** accepts already-redacted, canonically rendered bytes from
the [Editor Event Stream](../05-Editor-Event-Stream/README.md). It owns only the
private mechanics needed to route those bytes to configured diagnostic
destinations, such as a file or standard error.

Its descriptor declares contract identity/version; capability, owner, and
exact source Event Stream lifecycle identities; readiness predicate; carrier
safety; record, aggregate outstanding record/byte, submission-work, temporary-
byte, activation, and shutdown bounds; shutdown policy; and operations safe in
degraded readiness. Shutdown-report record and represented-source-byte limits
cover the outstanding capacities. Outstanding capacity includes every accepted
queued or in-flight record; moving one into a call does not release its charge.

Contract Authority supplies the output-record type identity and Carrier Safety
Descriptor. Construction rejects a different carrier identity or any schema-
version, ownership, borrow/copy/drop, lifetime, transfer/sharing, invocation/
unwind, mismatch, or unsafe-boundary difference from the exact declaration.
`DiagnosticRecordContract` groups these values; it grants no construction,
lifecycle, or submission authority.

## Record and submission

One output record is a nonzero, capacity-exact immutable owned byte carrier.
It is stamped with the source Event Stream lifecycle, Event sequence, and
target output lifecycle. Identity comes from the admitted Event record and is
never parsed from bytes. The move-only carrier permits immutable borrowing,
releases its storage, and cannot be cloned into another charge owner.

Descriptor validation checks source identity, size, arithmetic, and bounds and
returns only a checked allocation size. Runtime performs the bounded copy, then
record binding re-presents the same source reference and repeats validation.
Validation grants no construction, lifecycle, or submission authority. Binding
retains no source borrow. Failure returns the input carrier. Only an installed
output may take ownership; submission atomically revalidates target, source,
duplicate identity, and lifecycle without allocation, copying, parsing,
redaction, rendering, or I/O.

Negotiation fails before a grant with typed unsupported-contract or
incompatible-version outcome and leaves the carrier caller-owned. One granted
submission returns exactly one of:

- **accepted:** the adapter took ownership within declared bounds;
- **dropped:** no acceptance, with typed invalid-record, bound, readiness,
  pressure, charge-preparation, or live-output cause; or
- **closed:** Closing, Closed, replacement, or latched incomplete shutdown
  prevents acceptance by this generation.

Accepted transfers the exact carrier. Dropped and closed return it without a
retained borrow, pointer, or partial copy. The exact mapping is:

- foreign source/output identity, duplicate identity, or invalid metadata is
  `dropped(invalid-record)`;
- an out-of-descriptor record is `dropped(bound-exceeded)`;
- Constructed or Starting is `dropped(not-ready)`;
- non-waiting authority contention is `dropped(synchronization-pressure)`;
- record exhaustion is `dropped(record-capacity)`;
- byte exhaustion is `dropped(byte-capacity)`;
- aggregate overflow is `dropped(charge-overflow)`;
- failed but not Closing is `dropped(output-failed)` with typed
  destination-unavailable, health-unavailable, or internal cause; and
- Closing, Closed, replacement/stale generation, and latched incomplete
  shutdown are distinct closed causes.

Cancellation and timeout do not apply to inline submission. They apply to
accepted destination work and shutdown. Acceptance implies neither destination
write nor crash durability. Output failure never changes editing state or
synchronously calls into the Event Stream. Drops use bounded non-recursive
stream-owned counters. DiagnosticOutput never assigns Event sequence/time,
redacts, renders, retains the Event Ring, or creates another logging pipeline.

## Operation classifications

Submission, descriptor inspection, record validation, and binding are:

```text
NonBlockingInvocation + InlineTerminal + Single + LocalOnly
+ NoUserCallback + NonReentrant
```

Binding is allocation-free, retains no source reference, and traverses at most
the maximum record bytes. A private destination attempt is separately
`ExternalMutation` and preserves Effect Certainty and Outcome Cause.

Activation and installed-output shutdown are:

```text
CallerMayWait + DeferredTerminal + Single + LocalOnly
+ NoUserCallback + NonReentrant
```

Shutdown relays destination certainty/cause and grants no mutation authority.
Output-health operations are defined by the
[Bootstrap Health contract](05-Bootstrap-Health.md).

## Destination results

A destination result carries output and source Stream lifecycles, Event
sequence, total/confirmed bytes, Effect Certainty, Outcome Cause, and one cut.
Public construction proves value coherence, not submission or execution.

- `before-call`: no call for the record was ever authorized;
- `in-call`: at least one call was authorized and the latest lacks a
  trustworthy return; and
- `returned`: at least one call was authorized and every authorized call has a
  trustworthy return with confirmed-prefix accounting.

Stopping before another call after returned progress remains `returned`;
`StopBeforeNextCall` is private state. Paired closing/replacement, write-zero/
failure, and internal-before/in-call phrases name distinct causes.

Success is `returned + Applied + full prefix`. Closing/replacement discard is
`before-call + DefinitelyNotApplied + zero`. Cancellation, shutdown deadline,
and supervisor reaping derive certainty from the cut: before-call is
DefinitelyNotApplied/zero, in-call is OutcomeUnknown, and returned is
DefinitelyNotApplied at zero or OutcomeUnknown after strict partial progress.
Returned zero/failure/retry exhaustion follows the returned rule; writer-
contract violation is returned/OutcomeUnknown. Internal-before-call may be
before-call or returned and follows the same cut rule; internal-in-call is
in-call/OutcomeUnknown.

Confirmed prefix never exceeds nonzero total. OutcomeUnknown permits zero or
strict partial progress; only success permits full prefix. No trustworthy cut
means no coherent result. Unknown effects are never blindly retried. Flush,
sync, crash durability, and persistence are outside this contract.

Shutdown cuts, public values, exact relations, borrowing, and latching are
defined by [Diagnostic Shutdown Values](04-Diagnostic-Shutdown-Values.md).
