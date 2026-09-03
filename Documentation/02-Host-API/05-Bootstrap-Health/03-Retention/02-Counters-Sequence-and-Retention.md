# OutputHealth Counters, Sequence, and Retention

[Retention index](../03-Retention-Lease-and-Snapshot.md)

## Exact counter bank

OutputHealth has exactly 31 unsigned 64-bit saturating slots. Every slot
exposes whether saturation occurred. The seven fixed groups are:

1. `OutputHealthAcceptedCounters`: accepted records and record bytes;
2. `OutputHealthDropCounters`: all ten `DiagnosticDropCause` leaves;
3. `OutputHealthClosedCounters`: all four `DiagnosticClosedCause` leaves;
4. `OutputHealthEffectCounters`: `Applied`, `DefinitelyNotApplied`, and
   `OutcomeUnknown`;
5. `OutputHealthEvictionCounters`: anomaly records and charged bytes;
6. `OutputHealthRecordResultCounters`: pressure, invalid, stale, closed, and
   sequence exhaustion; and
7. `OutputHealthImportCounters`: accepted, rejected, dropped, closed, and
   attempt limit.

`OutputHealthCounters` contains those seven groups.
`OutputHealthRecordResultKind` and `OutputHealthImportCounterKind` name the
last two groups' closed leaves. Import leaves define counter schema only; they
do not grant an import operation.

Cause-keyed groups may be reconstructed from one fixed compile-time array in
the documented enum-declaration order. Public lookup uses the typed cause or
kind, never a numeric index. Dynamic cause maps, unknown buckets, reset,
decrement, and producer-selected counters are forbidden.

## Saturation

`SaturatingCounter::saturating_add` adds one unsigned scalar. Adding zero
preserves the value. An in-range sum, including exact arrival at `u64::MAX`,
does not set saturation. A later nonzero addition that cannot be represented
retains `u64::MAX` and sets saturation. Saturation never clears.

Saturation changes only the affected slot. It does not wrap, clear retained
evidence, invent success, consume a health sequence, or grant capacity.

## Proposal mapping

The [proposal contract](../02-Output-Health-Proposals.md) owns the complete
producer mapping:

- submission Accepted adds one accepted record and its exact record bytes;
- submission Dropped increments its exact ten-leaf drop slot;
- submission Closed increments its exact four-leaf closed slot;
- every destination result increments its Effect Certainty slot;
- destination Success is counter-only; and
- non-Success destination, activation, Runtime, and incomplete shutdown are
  anomaly candidates.

Eviction, record-result, and import counters are state-operation outcomes. A
producer cannot propose them. A successful counter-only proposal changes no
retained-anomaly capacity.

## Health sequence and retained range

`OutputHealthSequence` is a distinct checked nonzero `u64`. First is one.
Construction rejects zero. `next` fails at `u64::MAX` and never wraps or
reuses a value. It is not an `EventSequence`.

`OutputHealthSequencePosition` is `Next(OutputHealthSequence)` or `Exhausted`.
`OutputHealthHighWater` is empty or holds the latest assigned sequence and
derives the position after that complete state cut. Empty derives `Next(1)`;
latest `u64::MAX` derives `Exhausted`.

`OutputHealthRetainedRange` is empty or an inclusive ordered earliest/latest
pair. A nonempty latest equals high-water because only oldest records are
evicted. Eviction changes the earliest value, not high-water or a retained
record's sequence.

Only an accepted anomaly receives a sequence. Counter-only acceptance never
does, including after exhaustion. Sequence exhaustion latches and rejects
later anomaly retention while counter-only updates remain available.

## Complete anomaly and fixed charge

`OutputHealthAnomalyRecord` joins one assigned health sequence, one anomaly-
candidate `OutputHealthProposal`, and the descriptor's fixed anomaly charge.
Its constructor rejects a counter-only proposal and a source Stream mismatch.
The caller cannot supply the charge independently.

The four retained families are:

- destination failure with the complete destination result and cut evidence;
- activation failure with typed cause and rollback;
- Runtime failure with typed cause and execution; and
- incomplete shutdown with unresolved count and full owner state at latch.

The proposal already owns the exact copied scalars for each family. The
anomaly record retains no DiagnosticOutput record bytes or borrowed report.
Constructing it proves a coherent candidate, not state assignment or retention.

## Capacity and eviction

Accepted anomaly recording evicts oldest complete records until one fixed slot
fits. It never truncates or partly retains a record. Eviction adds exact record
and fixed-charge bytes to the two eviction counters.

The retained range has no holes. Evicted record count plus retained record
count equals assigned high-water. A saturated eviction-record counter is
incoherent because the finite sequence domain cannot assign more than
`u64::MAX` anomalies. Evicted bytes may saturate earlier; overflow requires
`u64::MAX` plus visible saturation.

No partition borrows Event Ring capacity or another health partition's bound.
The separate 1 MiB Event Ring ceiling does not apply to OutputHealth.
