# OutputHealth Owner Snapshot

[Retention index](../03-Retention-Lease-and-Snapshot.md)

## Public snapshot values

Kernel Interface owns:

- `OutputHealthSnapshotRequest`;
- `OutputHealthSnapshotLimits`;
- `OutputHealthSnapshotStatistics`;
- `OutputHealthSnapshotCoverage`;
- `OutputHealthSnapshotMetadata`; and
- `OutputHealthSnapshot<'a>`.

These values define a non-destructive owner observation. This contract does
not select the concrete caller-owned carrier or add the live owner snapshot
port. The first deterministic state consumer must close that carrier and port
without changing the value relations below.

## Request and caller charge

`OutputHealthSnapshotRequest::new` derives the exact source Stream from the
descriptor. The caller supplies its charge-owner lifecycle plus nonzero maximum
complete records and charged bytes. Both maxima fit the descriptor and the byte
maximum can fit at least one fixed anomaly charge.

The caller owns and charges any eventual output carrier. Snapshot allocates no
output and transfers no health-state ownership. Record and charged-byte values
are portable contract accounting, not Rust layout, allocator bookkeeping, or
RSS.

## Statistics and coverage

`OutputHealthSnapshotStatistics` contains exact copied complete-record and
charged-byte counts. Both fit the request. Zero records requires zero bytes and
vice versa.

`OutputHealthSnapshotCoverage` is exactly:

- `Complete`; or
- `Partial { first_uncopied: OutputHealthSequence }`.

Partial means capacity ended before the named complete record. It never means
a partial record. If the first retained record cannot fit, construction returns
`SnapshotBoundExceeded` rather than a zero-record Partial snapshot.

## Metadata join

`OutputHealthSnapshotMetadata` joins the request's source Stream and charge
owner with:

- assigned `OutputHealthHighWater`;
- current `OutputHealthRetainedRange`;
- the request-derived fixed anomaly charge;
- snapshot statistics;
- next `OutputHealthSequencePosition`; and
- coverage.

The join obeys these laws:

- empty high-water requires an empty retained range;
- nonempty retained latest equals assigned high-water;
- an empty retained range has zero copied records and Complete coverage;
- Complete copies the entire retained range and advances after high-water;
- Partial copies a nonempty retained prefix, names the first uncopied retained
  sequence, and uses that value as its next position; and
- Complete at high-water `u64::MAX` has next position `Exhausted`.

High-water remains present after complete eviction. Thus an empty retained
range distinguishes pristine or counter-only state from fully evicted anomaly
history. Counter-only updates change neither high-water nor retained range.

## Borrowed complete-record view

`OutputHealthSnapshot::new` accepts metadata, a borrowed complete
`OutputHealthCounters` value, and a borrowed slice of complete anomaly
records. It validates:

- slice count equals declared copied records;
- every record names the snapshot's exact source Stream generation;
- sequences are contiguous oldest-first from retained earliest;
- record coverage and first-uncopied position agree with metadata;
- the checked sum of fixed record charges equals declared copied bytes; and
- eviction counters, retained count, and high-water account for every assigned
  anomaly without a hole.

The snapshot exposes metadata and borrowed counters/complete records. It cannot
mutate, acknowledge, retry, import, clear, render, or allocate. Its Rust
lifetime protects both borrows; the complete Stream generation fences use
against live state.

Construction validates structural coherence only. It does not prove that a
live owner observed one atomic state cut, copied records, or granted caller
capacity. Phase D mechanism and shared contract tests must prove those facts.

## Import boundary

The first-import freeze, cursor, attempt, result, Event family, renderer,
admission, and startup-summary projection remain in the
[import contract](../04-Import-Readiness-and-Failure.md). Five counter labels
do not imply that any import operation exists.
