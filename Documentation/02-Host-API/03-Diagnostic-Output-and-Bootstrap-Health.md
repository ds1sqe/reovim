# Diagnostic Output and Bootstrap Health

[Host API index](../02-Host-API.md)

## DiagnosticOutput

**DiagnosticOutput** accepts already-redacted, canonically rendered bytes from
the [Editor Event Stream](../05-Editor-Event-Stream/README.md). It owns only the
private mechanics needed to route those bytes to configured diagnostic
destinations, such as a file or standard error.

Its descriptor declares contract identity/version; capability, owner, and
exact source Event Stream lifecycle identities; readiness predicate; carrier
safety; record, aggregate outstanding record/byte, submission-work, temporary-
byte, activation, and shutdown bounds; shutdown policy; and operations safe in
degraded readiness. Shutdown-report capacity is at least the outstanding-
record capacity. Outstanding capacity includes every accepted queued or in-
flight record; moving one into an output call does not release its charge.

Contract Authority supplies the output-record type identity and Carrier Safety
Descriptor. Construction rejects a different carrier type identity or any
schema-version, ownership, borrow/copy/drop, lifetime, transfer/sharing,
invocation/unwind, mismatch, or unsafe-boundary difference from the exact
move-only declaration below.

`DiagnosticRecordContract` groups those independently supplied values. It is
a declaration, not construction, lifecycle, or submission authority.

One output record is a nonzero, capacity-exact immutable owned byte carrier.
It is stamped with the full source Event Stream lifecycle identity, its Event
sequence, and the target DiagnosticOutput lifecycle identity. Identity is
supplied from the admitted Event record and is never recovered by parsing the
bytes. The carrier is move-only, permits immutable borrowing, releases its own
storage, and cannot be cloned into a second charge owner.

Descriptor validation checks source identity, size, arithmetic, and hard bounds
and returns only the checked allocation size. It grants no lifecycle or
submission authority and retains no Event-record borrow. Runtime performs the
bounded allocation/copy, then record binding re-presents the same scoped Event
reference and repeats validation before creating an exact-sized immutable
carrier. Any failure returns the same input carrier. Only an installed
DiagnosticOutput can take ownership; submission atomically revalidates target,
source, duplicate identity, and lifecycle without allocation, copying, parsing,
redaction, rendering, or I/O.

Contract negotiation fails before a grant with typed unsupported or
incompatible outcome and leaves the carrier caller-owned. One bounded granted
submission returns exactly one of:

- **accepted:** the adapter took ownership within its declared bounds;
- **dropped:** it did not accept the record, with a typed invalid-record,
  bound-exceeded, not-ready, pressure, charge-preparation, or live-output
  failure cause; or
- **closed:** Closing, Closed, replacement, or latched incomplete shutdown
  prevents this output generation from accepting records.

Accepted transfers ownership of the exact byte carrier to DiagnosticOutput.
Dropped and closed return ownership to the caller without retaining a borrow,
pointer, or partial copy.

The exact mapping is:

- another source/output identity, duplicate identity, or invalid metadata is
  `dropped(invalid-record)`;
- a record outside the descriptor is `dropped(bound-exceeded)`;
- a Constructed or Starting output is `dropped(not-ready)`;
- non-waiting authority contention is `dropped(synchronization-pressure)`;
- outstanding-record and outstanding-byte exhaustion are respectively
  `dropped(record-capacity)` and `dropped(byte-capacity)`;
- aggregate overflow is `dropped(charge-overflow)`;
- a failed but not Closing output is `dropped(output-failed)`, with a typed
  destination-unavailable, health-unavailable, or internal cause;
- Closing, Closed, same-output replacement/stale generation, and latched
  incomplete shutdown are distinct closed causes; and
- cancellation and timeout do not apply to the inline submission call. They
  apply only to already accepted destination work and shutdown.

Accepted does not imply crash durability. Output failure never changes an
editing result and must not synchronously call back into the Event Stream.
Every drop is visible through non-recursive bounded counters owned by the
stream. Closing stops new admission; shutdown has a bounded drain-or-discard
outcome and is idempotent.

DiagnosticOutput must not assign Event sequence or timestamp, redact, render,
retain the Event Ring, or create another logging pipeline.

## Operation classifications

Submission is exactly:

```text
NonBlockingInvocation + InlineTerminal + Single + LocalOnly
+ NoUserCallback + NonReentrant
```

Its terminal effect is a bounded local ownership transfer. A private
destination attempt is a separately classified `ExternalMutation` that
preserves Effect Certainty and Outcome Cause. Submission never claims that the
destination wrote, flushed, or durably retained the record.

Descriptor inspection, pre-allocation validation, and final record binding use
the same LocalOnly inline operation shape. Binding traverses at most the
declared maximum record bytes, allocates zero bytes, and retains no source
reference. Its byte bound is therefore also its traversal-work bound.

Activation and shutdown are:

```text
CallerMayWait + DeferredTerminal + Single + LocalOnly
+ NoUserCallback + NonReentrant
```

Shutdown relays per-record destination certainty and cause but grants no new
destination-mutation authority. Output-health record and snapshot operations
are bounded LocalOnly operations with all six axes declared.

## Destination and shutdown outcomes

A private destination result carries exact output lifecycle, source Stream
lifecycle, Event sequence, total/confirmed bytes, Effect Certainty, Outcome
Cause, and one authorization cut. Public value construction proves coherence,
not submission acceptance or destination execution.

Cuts are disjoint:

- `before-call` means no call for the record was ever authorized;
- `in-call` means at least one was authorized and the latest has no trustworthy
  return; and
- `returned` means at least one was authorized and every authorized call has a
  trustworthy return and confirmed-prefix accounting.

After any returned progress, stopping before another call remains `returned`;
`StopBeforeNextCall` is private mechanism state, not a result cut. Paired
closing/replacement, write-zero/failure, and internal-before/in-call phrases
name distinct causes.

Success is `returned + Applied + full prefix`. Closing/replacement discard is
`before-call + DefinitelyNotApplied + zero`. Cancellation, shutdown deadline,
and supervisor reaping derive certainty from the cut: before-call is
definitely-not-applied/zero, in-call is outcome-unknown, and returned is
definitely-not-applied at zero or outcome-unknown after progress. Returned
write-zero/failure/retry exhaustion follow that returned rule; writer-contract
violation is returned/unknown. Internal-before-call may be before-call or
returned and follows the cut rule; internal-in-call is in-call/unknown.

Confirmed prefix never exceeds the nonzero total. OutcomeUnknown permits zero
or a strict partial prefix; only success has a full prefix. Without trustworthy
cut evidence no coherent result exists. Unknown effects are never blindly
retried. Flush, sync, crash durability, and persistence remain outside scope.

The shutdown report covers exactly the records still owned at the first
Closing cut: queued plus in-flight. Its capacity is at least the outstanding-
record capacity, so it cannot truncate this set. Entries are ordered by source
Event sequence. Per-record and aggregate count/byte evidence is therefore
bounded by the descriptor's outstanding capacities; it does not retain every
historical accepted record. Each entry carries work disposition, execution
terminality, confirmed prefix, certainty, and destination cause.

A discarded record is `NeverStarted + DefinitelyNotApplied` with zero
confirmed prefix. An unresolved record is
`OwnedNonterminal + OutcomeUnknown` and requires `IncompleteShutdown`. A
drained record has terminal execution and may be `Applied`,
`DefinitelyNotApplied`, or `OutcomeUnknown` according to the
destination evidence. `Applied` requires success and a full confirmed prefix;
`DefinitelyNotApplied` requires zero prefix. OutcomeUnknown permits zero or a
strict partial prefix. Report counts equal the exact disposition counts, and
their checked sum equals the complete record sequence length.

`Closed` requires no unresolved record and release-complete output execution.
At the shutdown cut, any unresolved record or nonterminal output owner requires
`IncompleteShutdown`; the latter may occur with zero unresolved records. The
report is immutable and latched. Repeated shutdown returns the same report.
Later ownership resolution is separately monotonic:
`live-owned -> exited-owned -> joined-owned`, or
`live-owned -> containing-process-reaped`. It reclaims ownership but never
rewrites the report or upgrades incomplete shutdown to Closed.

## Non-recursive bootstrap health

Clock, Event Stream, and DiagnosticOutput bootstrap and failure reporting must
not recursively require the failed capability. One fixed bounded Event
Stream-owned bootstrap-health state records failure before the Event Stream is
ready or while output is unavailable. OutputHealth is an output-specific
projection into that one state, not another health store. The startup
proposal's `clear`, `imported`, and `pending` value is only a summary marker;
it is not the state, lease, or snapshot.

This chapter owns only the OutputHealth partition. Clock and Event Stream
projections require separately versioned schemas. Every partition remains
inside the same aggregate record and resident-byte bounds.

Its descriptor names one Event Stream owner, schema/version, anomaly record
and resident-byte bounds, exact 31-slot counter bank, lifecycle/order,
caller-owned snapshot record/byte limits, and bounded import attempts. Resident
charge includes counters, retained records, indexes, and reserved capacity.

The 31 slots are: accepted records/bytes (2); ten dropped leaves; four closed
leaves; Applied/DefinitelyNotApplied/OutcomeUnknown (3); evicted anomaly
records/bytes (2); health pressure, invalid, stale, closed, and sequence-
exhaustion results (5); and accepted, rejected, dropped, closed, and attempt-
limit import results (5). Every slot is unsigned 64-bit, saturating, and
exposes saturation. Success changes counters only.

Retained anomalies are exactly destination failure with record/prefix/effect
evidence, activation failure with rollback, runtime failure with execution, or
incomplete shutdown with unresolved count and owner execution. Detailed non-
success causes live in those records rather than a dynamic counter map.

Each retained anomaly receives one checked nonzero health sequence after a
successful record operation. That sequence belongs only to health snapshot and
import ordering; it is never an Event sequence. Records are oldest-first.
Capacity evicts oldest complete anomaly records and increments exact eviction
counters. Sequence exhaustion latches, never wraps or reuses a value, and
rejects later records while counters remain available. A pressured record uses
a separate fixed non-waiting counter path while health remains live. After
Closed, the attempt returns Closed and does not claim a terminal-state update.

Assembly wires an adapter-facing write-only `OutputHealthLease`. Kernel Runtime,
as Event Stream Lifecycle Owner, exclusively retains snapshot/import authority.
The lease is generation-bound and records only typed output updates. It cannot
snapshot, import, render, admit an Event, invoke a callback, or reach an Event
Stream object. The owner snapshot is non-destructive and copies complete
records into caller-owned bounded capacity; it cannot update or clear state.
Both operations are:

```text
NonBlockingInvocation + InlineTerminal + Single + LocalOnly
+ NoUserCallback + NonReentrant
```

After Closing, the retired-generation lease remains usable only for its already
accepted work, including while that work is failed-owned, until terminality or
process reaping. This preserves final visibility without new submission
authority.

The first import attempt freezes the health high-water mark and loss counters.
Its input is one nonzero loss summary, then retained anomalies through that
mark in health-sequence order. Only accepted Event admission advances the
cursor; another outcome leaves the entry pending. Accepted entries are never
replayed, later records are excluded, and a fixed aggregate attempt limit
halts further work as pending. Import never erases health state. Imported
Events preserve health provenance but receive new Event Stream sequences; a
live importer still requires its own ratified Event family and renderer.

Failure to admit the one startup summary does not change an editing result or
silently block readiness. A required Event Stream that fails its own readiness
predicate prevents Kernel readiness. If the stream is Ready but startup
summary admission fails, the Lifecycle Owner may publish only the descriptor's
explicit diagnostics-degraded Ready result, naming the fixed health cause and
remaining safe operations.
