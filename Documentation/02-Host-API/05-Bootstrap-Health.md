# Bootstrap Health

[Host API index](../02-Host-API.md)

## Non-recursive failure state

Clock, Event Stream, and
[DiagnosticOutput](03-Diagnostic-Output.md) bootstrap and failure reporting
must not recursively require the failed capability. One fixed bounded Event
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
Closed, the attempt returns Closed and claims no terminal-state update.

Assembly wires an adapter-facing write-only `OutputHealthLease`. Kernel
Runtime, as Event Stream Lifecycle Owner, exclusively retains snapshot/import
authority. The lease is generation-bound and records only typed output
updates. It cannot snapshot, import, render, admit an Event, invoke a callback,
or reach an Event Stream object. The owner snapshot is non-destructive and
copies complete records into caller-owned bounded capacity; it cannot update
or clear state. Both operations are:

```text
NonBlockingInvocation + InlineTerminal + Single + LocalOnly
+ NoUserCallback + NonReentrant
```

After Closing, the retired-generation lease remains usable only for already
accepted work, including failed-owned work, until terminality or process reap.
This preserves final visibility without new submission authority.

The first import attempt freezes the health high-water mark and loss counters.
Its input is one nonzero loss summary, then retained anomalies through that
mark in health-sequence order. Only accepted Event admission advances the
cursor; another outcome leaves the entry pending. Accepted entries are never
replayed, later records are excluded, and a fixed aggregate attempt limit
halts further work as pending. Import never erases health state. Imported
Events preserve health provenance but receive new Event Stream sequences; a
live importer still requires its own ratified Event family and renderer.

Failure to admit the startup summary neither changes editing state nor silently
blocks readiness. A required Event Stream that fails its readiness predicate
prevents Kernel readiness. If the stream is Ready but summary admission fails,
the Lifecycle Owner may publish only the descriptor's explicit diagnostics-
degraded Ready result, naming the fixed health cause and remaining safe
operations.
