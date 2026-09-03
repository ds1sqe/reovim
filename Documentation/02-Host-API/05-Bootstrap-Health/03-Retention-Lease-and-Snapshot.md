# OutputHealth Retention, Lease, and Snapshot

[Bootstrap Health index](../05-Bootstrap-Health.md)

## Retained anomaly records

Retained anomalies have exactly four families:

- destination failure with complete destination result, including record,
  confirmed-prefix, Effect Certainty, cause, and cut evidence;
- activation failure with output/source identity, typed cause, and rollback;
- Runtime failure with output/source identity, typed cause, and execution; and
- incomplete shutdown with output/source identity, unresolved count, and full
  output-owner state at latch.

Detailed non-success causes live in complete typed records rather than a
dynamic counter map. Submission rejection and destination Success remain
counter-only as defined by
[OutputHealth proposals](02-Output-Health-Proposals.md).

Each retained anomaly receives one checked nonzero health sequence only after
a successful record operation. That sequence belongs solely to health snapshot
and import ordering; it is never an Event sequence and is never supplied by a
producer. Records are oldest-first.

Capacity evicts oldest complete anomaly records and increments exact eviction
record/byte counters. It never retains a partial record. Sequence exhaustion
latches, never wraps or reuses a value, and rejects later anomaly retention
while counter-only updates remain available.

A pressured operation uses a separate fixed non-waiting result-counter path
while health remains live. After Closed, an attempt returns Closed and claims
no terminal-state update.

## Write-only lease

Assembly wires an adapter-facing write-only `OutputHealthLease`. Kernel
Runtime, as Event Stream Lifecycle Owner, exclusively retains snapshot and
import authority.

The lease is generation-bound and records only typed
`OutputHealthProposal` values. It cannot snapshot, import, render, admit an
Event, invoke a callback, or reach an Event Stream object. An adapter cannot
mutate counters or choose a health sequence directly.

After Closing, a retired-generation lease remains usable only for already
accepted work, including failed-owned work, until terminality or containing-
process reap. It grants no new DiagnosticOutput submission authority. This
preserves final visibility without accepting new output work.

The lease record operation is:

```text
NonBlockingInvocation + InlineTerminal + Single + LocalOnly
+ NoUserCallback + NonReentrant
```

It returns one typed state-owned result for accepted, pressure, invalid,
stale, closed, or sequence-exhaustion behavior. Its exact values and atomic
counter/retention relation must be closed with the state contract; proposal
construction alone does not define them.

## Owner snapshot

The owner snapshot is non-destructive and copies complete anomaly records into
caller-owned bounded capacity. It names exact coverage, loss/eviction state,
counter saturation, and the health-sequence interval represented. It cannot
update, acknowledge, retry, import, or clear health state.

Snapshot is:

```text
NonBlockingInvocation + InlineTerminal + Single + LocalOnly
+ NoUserCallback + NonReentrant
```

Snapshot record and byte limits come from the descriptor. The caller provides
and charges output capacity; insufficient capacity returns a typed partial or
rejected result fixed by the snapshot value contract. No allocation-returning
unbounded collection view is public.
