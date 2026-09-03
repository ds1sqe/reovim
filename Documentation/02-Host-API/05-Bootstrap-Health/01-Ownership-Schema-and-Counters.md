# Bootstrap-Health Ownership, Schema, and Bounds

[Bootstrap Health index](../05-Bootstrap-Health.md)

## One non-recursive state

Clock, Event Stream, and
[DiagnosticOutput](../03-Diagnostic-Output.md) bootstrap and failure reporting
must not recursively require the failed capability. One fixed bounded Event
Stream-owned bootstrap-health state records failure before the Event Stream is
ready or while output is unavailable.

OutputHealth is an output-specific partition in that one state. It is not
another Host Capability, output-owned health store, Event Ring, or diagnostic
pipeline. The startup proposal's `clear`, `imported`, and `pending` value is
only a summary marker; it is not the state, lease, or snapshot.

## Aggregate descriptor and bounds

The aggregate descriptor names its Event Stream Lifecycle Owner, contract and
schema versions, aggregate record and resident-byte bounds, partition layout,
lifecycle/order, caller-owned snapshot record/byte limits, and bounded import
attempts.

The OutputHealth partition fixes its proposal contract, descriptor, anomaly-
record shape, retained-record and resident-byte bounds, counter-bank schema,
and sequence space in the
[retention contract](03-Retention-Lease-and-Snapshot.md). Resident charge
includes counters, retained complete records, indexes, and reserved capacity.
No partition double-charges shared aggregate metadata or borrows capacity
invisibly from another partition.
