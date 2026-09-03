# Diagnostic Shutdown Values

[Host API index](../02-Host-API.md) ·
[Diagnostic Output](03-Diagnostic-Output.md)

## Scope and two cuts

Installed-output shutdown has two distinct linearization cuts:

1. The **first-Closing membership cut** closes admission and fixes the exact
   queued plus in-flight record identities owned by the output.
2. The **terminal report-latch cut** records their final entries, statistics,
   output-owner state, and outcome after the bounded attempt.

The canonical immutable report covers the complete first-cut set. Its record
and represented-source-byte capacities cover outstanding capacities and cannot
truncate that set. It is latched once for the output lifecycle generation.
Repeated shutdown returns the same semantic report; pointer identity is not a
contract.

A public value constructor instead validates one supplied ordered candidate.
The output lifecycle scopes it and is the future canonical latch key. Value
construction proves neither exact first-cut membership, output ownership,
destination execution, historical cuts, nor one-time latching.

## Output owner state

`DiagnosticOutputOwnerState` has exactly:

- `NoOwnedExecution`;
- `LiveOwned`;
- `ExitedOwned`;
- `JoinedOwned`; and
- `ContainingProcessReaped`.

It derives `Clone + Copy + Debug + PartialEq + Eq`. Its `pub const fn`
`execution_is_terminal` and `release_is_complete` predicates have this matrix:

| State | Terminal | Release complete |
| --- | --- | --- |
| `NoOwnedExecution` | yes | yes |
| `LiveOwned` | no | no |
| `ExitedOwned` | yes | no |
| `JoinedOwned` | yes | yes |
| `ContainingProcessReaped` | yes | yes |

The state exhaustively summarizes the independently releasable output
execution owner and its release obligation able to retain an accepted record
or canonical report backing. It does not summarize unrelated destination-
resource closure. `LiveOwned` means owning execution is not proved terminal.
`ExitedOwned` means it is terminal but its join/release remains owned.
`NoOwnedExecution` means this realization has no such independent owner and no
such obligation remains; completed work may still appear in its report.

Later ownership observation is reflexive and follows only:

```text
LiveOwned -> ExitedOwned -> JoinedOwned
LiveOwned -> ContainingProcessReaped
```

NoOwnedExecution, JoinedOwned, and ContainingProcessReaped are terminal.
ExitedOwned advances only to JoinedOwned. An observer may skip ExitedOwned on
the joined branch. Later resolution never rewrites or upgrades the latch.

## Entry value and errors

`DiagnosticShutdownEntry` contains one `DiagnosticDestinationResult`,
`WorkDisposition`, and `ExecutionOutcome`. It derives
`Clone + Copy + Debug + PartialEq + Eq`. Its `pub const fn new` accepts those
values in that order. Accessors are `destination`, `disposition`, and
`execution`.

```text
DiagnosticShutdownEntry::new(
    DiagnosticDestinationResult,
    WorkDisposition,
    ExecutionOutcome,
) -> Result<DiagnosticShutdownEntry, DiagnosticShutdownEntryError>
```

Construction returns the first failure in this exact order:

1. disposition/execution mismatch;
2. execution/destination mismatch;
3. disposition/destination mismatch; then
4. supervisor-reaping/execution mismatch.

The relation is exact:

- `NeverStarted` always requires `BeforeCall`;
- `Drained` requires terminal execution;
- `Drained + NeverStarted` permits only `Cancellation` or
  `InternalFailureBeforeCall`, both at `BeforeCall`;
- another terminal Drained entry preserves any coherent destination result
  except `ClosingDiscard` or `ReplacementDiscard`;
- `Discarded` requires `NeverStarted + BeforeCall + DefinitelyNotApplied`
  with zero prefix and exactly `ClosingDiscard`, `ReplacementDiscard`, or
  `ShutdownDeadline`;
- `Unresolved` requires `OwnedNonterminal` and preserves any coherent non-
  discard destination result, including Applied, DefinitelyNotApplied, or
  OutcomeUnknown; and
- `SupervisorReaping` requires `ContainingProcessReaped`, while reaped
  execution may preserve another result established before reaping.

Thus an unresolved entry can be BeforeCall/DefinitelyNotApplied, Returned/
Applied, Returned/DefinitelyNotApplied, or InCall/OutcomeUnknown when its
already-coherent destination evidence says so. Execution state never erases
Effect Certainty.

`DiagnosticShutdownEntryError` derives
`Clone + Copy + Debug + PartialEq + Eq` and implements `Display + Error`. Its
exact variants are:

- `DispositionExecutionMismatch { disposition, execution }`;
- `ExecutionDestinationMismatch { execution, cut }`;
- `DispositionDestinationMismatch { disposition, execution, certainty, cause,
  cut, confirmed_prefix }`; and
- `ReapingExecutionMismatch { execution }`.

Entries retain copied scalars only, never source record bytes or a record
borrow. A source record may move or be released after its destination result is
constructed.

## Derived statistics

`DiagnosticShutdownStatistics` has no public constructor. Report validation
derives exact CheckedCount and CheckedByteCount values for Drained, Discarded,
Unresolved, and total records and source bytes. Its `pub const fn` accessors are
`drained_records`, `drained_source_bytes`, `discarded_records`,
`discarded_source_bytes`, `unresolved_records`, `unresolved_source_bytes`,
`total_records`, and `total_source_bytes`.

The type derives `Clone + Copy + Debug + PartialEq + Eq`. Counts and bytes form
three exact partitions; totals are their checked sums. Source bytes sum each
destination result's `total_bytes`. They are historical accounting, not
retained payload ownership. Confirmed prefixes remain per-entry.

## Borrowed candidate report

`DiagnosticShutdownReportRef<'a>` derives
`Clone + Copy + Debug + PartialEq + Eq`. The non-const `new` accepts, in
order, a `&DiagnosticOutputDescriptor`, `DiagnosticOutputOwnerState`, and
`&'a [DiagnosticShutdownEntry]`. It derives output and source Stream identity,
policy, statistics, and `ShutdownOutcome`; callers supply none of those
independently.

```text
DiagnosticShutdownReportRef::new(
    &DiagnosticOutputDescriptor,
    DiagnosticOutputOwnerState,
    &'a [DiagnosticShutdownEntry],
) -> Result<DiagnosticShutdownReportRef<'a>, DiagnosticShutdownReportError>
```

Its accessors are `output`, `source_stream`, `policy`, `outcome`,
`owner_at_latch`, `statistics`, and `entries`. Empty entries are valid and
retain exact descriptor identities and policy.

After entry-count conversion and capacity checking, validation traverses the
slice once in order. For each entry it checks output identity, source Stream
identity, per-record source-byte capacity, strictly increasing Event sequence,
and policy/discard relation, then performs the pre-add aggregate-byte check and
checked partition additions. The first failing check on the first failing
entry wins. Sequence gaps are valid; duplicates and reversal are not. After
traversal, any Unresolved entry requires `LiveOwned`.

Discard policy permits Discarded only with ClosingDiscard or
ReplacementDiscard. Drain policy permits Discarded only with ShutdownDeadline.
Cancellation or internal-before-call terminalization is Drained.

`DiagnosticShutdownReportError` derives
`Clone + Copy + Debug + PartialEq + Eq` and implements `Display + Error`. Its
exact variants are:

- `EntryCountUnrepresentable`;
- `RecordCapacityExceeded { requested, maximum }`;
- `OutputMismatch { sequence, expected, actual }`;
- `SourceStreamMismatch { sequence, expected, actual }`;
- `RecordByteCapacityExceeded { sequence, bytes, maximum }`;
- `SequenceNotIncreasing { previous, current }`;
- `PolicyDiscardMismatch { sequence, policy, cause }`;
- `SourceByteCapacityExceeded { sequence, accumulated, next, maximum }`; and
- `UnresolvedOwnerMismatch { owner, unresolved }`.

Identity payloads are LifecycleIdentity; sequence payloads are EventSequence;
count/byte payloads use their checked types. The source-byte check compares
`next` with remaining capacity before addition. Therefore public small-carrier
tests induce capacity exhaustion without arithmetic overflow. The unit
`EntryCountUnrepresentable` is the sole error without scalar payload and is
unreachable when supported `usize` fits `u64`.

Outcome is `Closed` exactly when the Unresolved count is zero and owner release
is complete. Otherwise it is `IncompleteShutdown`. Zero entries with LiveOwned
or ExitedOwned remain incomplete. A reaped entry has no universal relation to
the aggregate owner state; a mechanism proves that realization's history.

## Operation, borrowing, and mechanism boundary

Candidate construction traverses at most the prechecked shutdown-report record
limit and is exactly:

```text
NonBlockingInvocation + InlineTerminal + Single + LocalOnly
+ NoUserCallback + NonReentrant
```

It allocates, waits, and performs I/O zero times. Callee execution retains no
borrow. The returned value has one caller-retained immutable borrow; the caller
owns and charges its backing for `'a`, and shallow copies add no backing owner.

A canonical mechanism reserves and charges output-owned report storage before
Ready and exposes immutable views only after latching. Its resident-storage
byte bound is separate from represented source-record bytes. Unresolved source
carriers retain outstanding charge until terminal resolution or process reap.
Values alone prove none of reservation, complete membership, admission races,
execution, join/reap, repeated return, or later monotonic history. Public value
implementation and tests alone do not satisfy `HOST-003`.
