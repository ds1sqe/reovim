# OutputHealth Proposals

[Bootstrap Health index](../05-Bootstrap-Health.md)

## Pre-sequence write vocabulary

`OutputHealthProposal` is the complete DiagnosticOutput-originated input to a
future OutputHealth record operation. It is a coherent immutable proposal, not
proof that state accepted or recorded it. It contains no health sequence.

The sibling Kernel Interface `output_health` module owns exactly these public
proposal types:

- `OutputHealthRecordStamp`;
- `OutputHealthProposalKind`;
- `OutputHealthActivationFailureCause`;
- `OutputHealthRuntimeFailureCause`;
- `OutputHealthProposal`; and
- `OutputHealthProposalError`.

All derive `Clone + Copy + Debug + PartialEq + Eq`.
`OutputHealthProposalError` also implements `Display + Error`. These values
own copied scalars only. They allocate, wait, perform I/O, retain bytes, and
grant state authority zero times.

`OutputHealthProposal` has private payload state. Public constructors derive
common identities from one coherent source rather than accepting duplicated
caller fields.

## Record stamp

`OutputHealthRecordStamp::from_record(&DiagnosticOutputRecord)` copies the
output lifecycle, source Event Stream lifecycle, Event sequence, and exact
checked record-byte count. Its `output`, `source_stream`, `sequence`, and
`record_bytes` accessors are `pub const fn`. It retains no bytes or record
borrow.

## Categories and constructors

`OutputHealthProposalKind` has exactly:

- `SubmissionAccepted`;
- `SubmissionDropped`;
- `SubmissionClosed`;
- `DestinationResult`;
- `ActivationFailure`;
- `RuntimeFailure`; and
- `IncompleteShutdown`.

`OutputHealthProposal` exposes:

```text
submission_accepted(&DiagnosticOutputRecord) -> Self
submission_dropped(&DiagnosticOutputRecord, DiagnosticDropCause) -> Self
submission_closed(&DiagnosticOutputRecord, DiagnosticClosedCause) -> Self
destination_result(DiagnosticDestinationResult) -> Self
activation_failure(
    &DiagnosticOutputDescriptor,
    OutputHealthActivationFailureCause,
    RollbackOutcome,
) -> Self
runtime_failure(
    &DiagnosticOutputDescriptor,
    OutputHealthRuntimeFailureCause,
    ExecutionOutcome,
) -> Result<Self, OutputHealthProposalError>
incomplete_shutdown(
    DiagnosticShutdownReportRef<'_>,
) -> Result<Self, OutputHealthProposalError>
```

The submission constructors derive one record stamp. Destination result
already contains coherent identity and effect scalars. Activation and Runtime
derive output and source Stream identities from the descriptor. Incomplete
shutdown derives all fields from the report.

## Activation and Runtime failures

`OutputHealthActivationFailureCause` has exactly:

- `DestinationUnavailable`;
- `HealthUnavailable`;
- `ExecutionStartFailure`; and
- `InternalFailure`.

Every activation cause permits both `RollbackOutcome::Reverted` and
`RollbackOutcome::IncompleteShutdown`. Rollback is cleanup disposition, not
the primary failure cause.

`OutputHealthRuntimeFailureCause` has exactly:

- `DestinationUnavailable`;
- `HealthUnavailable`;
- `UnexpectedExecutionTermination`; and
- `InternalFailure`.

Runtime failure is post-activation. `ExecutionOutcome::NeverStarted` is
invalid for every Runtime cause. `UnexpectedExecutionTermination` requires
`Stopped` or `ContainingProcessReaped`. The other causes permit
`OwnedNonterminal`, `Stopped`, or `ContainingProcessReaped`.

The cause types are semantic translations. Environment errors, writer
handles, thread/join types, panic payloads, and platform codes do not cross the
contract.

## Incomplete shutdown

An incomplete-shutdown proposal copies output and source Stream identities,
the exact unresolved-record count, and full `DiagnosticOutputOwnerState` from
one `DiagnosticShutdownReportRef`.

Every report whose derived outcome is `IncompleteShutdown` is valid. This
includes zero unresolved records with `LiveOwned` or `ExitedOwned`. A report
whose outcome is `Closed` returns
`OutputHealthProposalError::ShutdownComplete`.

## Runtime validation error

`OutputHealthProposalError::RuntimeExecutionMismatch { cause, execution }`
retains both rejected values. It is returned for `NeverStarted` or for
`UnexpectedExecutionTermination + OwnedNonterminal`.

No other proposal constructor accepts a relation that requires validation.
There is no caller-supplied identity or byte count to mismatch.

## Access and mapping

The proposal exposes `kind`, `output`, `source_stream`, and
`is_anomaly_candidate`. Category-specific `pub const fn` accessors return
`Option` for record stamp, drop cause, closed cause, destination result,
activation cause, rollback, Runtime cause, execution, unresolved records, and
owner at latch.

The exact state-input mapping is:

- submission Accepted increments accepted records and bytes from its stamp;
- Dropped increments the counter for its exact ten-leaf cause;
- Closed increments the counter for its exact four-leaf cause;
- every destination result increments its Effect Certainty counter;
- destination Success is counter-only;
- every non-Success destination result is an anomaly candidate; and
- activation failure, Runtime failure, and incomplete shutdown are always
  anomaly candidates.

Thus `is_anomaly_candidate` is false exactly for the three submission
categories and destination Success. Actual retention still depends on the
future record operation and capacity.

## Timing and proof limit

An accepted-submission proposal is derived while the record remains borrowed,
before it moves into `DiagnosticOutput::submit`, and is used only if the result
is Accepted. This value contract cannot prove that historical correspondence.

Construction proves scalar derivation and relation validity only. It does not
prove submission, destination execution, lifecycle occurrence, OutputHealth
acceptance, counters, retention, health sequence, eviction, import, or startup-
summary projection.
