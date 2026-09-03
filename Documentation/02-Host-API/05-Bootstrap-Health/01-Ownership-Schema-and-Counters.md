# Bootstrap-Health Ownership, Schema, and Counters

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

## Descriptor and bounds

The aggregate descriptor names its Event Stream Lifecycle Owner, contract and
schema versions, aggregate record and resident-byte bounds, partition layout,
lifecycle/order, caller-owned snapshot record/byte limits, and bounded import
attempts.

The OutputHealth partition fixes its proposal contract, anomaly-record shape,
retained-record and resident-byte bounds, exact counter-bank schema, and
sequence space. Resident charge includes counters, retained complete records,
indexes, and reserved capacity. No partition double-charges shared aggregate
metadata or borrows capacity invisibly from another partition.

## Exact OutputHealth counter bank

OutputHealth has exactly 31 unsigned 64-bit saturating counter slots. Every
slot exposes whether saturation occurred. Dynamic cause-keyed counter maps are
forbidden.

The slots are:

- accepted records and accepted record bytes: two;
- all ten `DiagnosticDropCause` leaves: ten;
- all four `DiagnosticClosedCause` leaves: four;
- `Applied`, `DefinitelyNotApplied`, and `OutcomeUnknown`: three;
- evicted anomaly records and evicted anomaly bytes: two;
- health pressure, invalid, stale, closed, and sequence-exhaustion results:
  five; and
- accepted, rejected, dropped, closed, and attempt-limit import results: five.

The complete mapping from producer proposals to the first 19 slots is defined
by [OutputHealth proposals](02-Output-Health-Proposals.md). Eviction, health-
operation, and import counters are outcomes owned by their corresponding state
operations; a producer cannot propose them.

Saturation changes only the affected slot's saturation state and maximum
value. It does not wrap, clear retained evidence, invent success, or grant
capacity. A successful counter-only proposal changes no retained-anomaly
capacity.
