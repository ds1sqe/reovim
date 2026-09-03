//! Contract values for the Event Stream-owned `OutputHealth` partition.

use core::fmt;

use crate::bound::{BoundDescriptor, BoundError};
use crate::cancellation::ExecutionOutcome;
use crate::diagnostic::{
    CheckedByteCount, CheckedCount, DiagnosticClosedCause,
    DiagnosticDestinationCause, DiagnosticDestinationResult,
    DiagnosticDropCause, DiagnosticOutputDescriptor,
    DiagnosticOutputOwnerState, DiagnosticOutputRecord,
    DiagnosticShutdownReportRef,
};
use crate::event::{EventSequence, SaturatingCounter};
use crate::lifecycle::{LifecycleIdentity, RollbackOutcome, ShutdownOutcome};
use crate::operation::ContractSelection;

mod counters;
mod record;
mod snapshot;

/// Copied scalar identity and accounting from one Diagnostic Output record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutputHealthRecordStamp {
    output: LifecycleIdentity,
    source_stream: LifecycleIdentity,
    sequence: EventSequence,
    record_bytes: CheckedByteCount,
}

impl OutputHealthRecordStamp {
    /// Copies one record's identity and byte accounting without retaining it.
    #[must_use]
    pub const fn from_record(record: &DiagnosticOutputRecord) -> Self {
        Self {
            output: record.output(),
            source_stream: record.stream(),
            sequence: record.sequence(),
            record_bytes: record.size(),
        }
    }

    /// Returns the target Diagnostic Output lifecycle.
    #[must_use]
    pub const fn output(self) -> LifecycleIdentity {
        self.output
    }

    /// Returns the source Event Stream lifecycle.
    #[must_use]
    pub const fn source_stream(self) -> LifecycleIdentity {
        self.source_stream
    }

    /// Returns the source Event sequence.
    #[must_use]
    pub const fn sequence(self) -> EventSequence {
        self.sequence
    }

    /// Returns the exact checked canonical record bytes.
    #[must_use]
    pub const fn record_bytes(self) -> CheckedByteCount {
        self.record_bytes
    }
}

/// Closed semantic category of one `OutputHealth` proposal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputHealthProposalKind {
    /// Diagnostic Output accepted record ownership.
    SubmissionAccepted,
    /// A live output returned record ownership with a typed drop cause.
    SubmissionDropped,
    /// Lifecycle fencing returned record ownership with a closed cause.
    SubmissionClosed,
    /// One private destination attempt produced coherent effect evidence.
    DestinationResult,
    /// Diagnostic Output activation failed and attempted rollback.
    ActivationFailure,
    /// A previously activated Diagnostic Output failed while live.
    RuntimeFailure,
    /// Bounded shutdown latched incomplete execution or release.
    IncompleteShutdown,
}

/// Exact semantic cause of Diagnostic Output activation failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputHealthActivationFailureCause {
    /// The configured diagnostic destination could not be activated.
    DestinationUnavailable,
    /// The required `OutputHealth` readiness path was unavailable.
    HealthUnavailable,
    /// Required private output execution could not reach readiness.
    ExecutionStartFailure,
    /// Another bounded adapter-internal activation failure occurred.
    InternalFailure,
}

/// Exact semantic cause of failure after Diagnostic Output activation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputHealthRuntimeFailureCause {
    /// The ready diagnostic destination became unavailable.
    DestinationUnavailable,
    /// Required output-health accounting became unavailable.
    HealthUnavailable,
    /// Required private execution terminated outside expected Closing.
    UnexpectedExecutionTermination,
    /// Another bounded live-output internal failure occurred.
    InternalFailure,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OutputHealthProposalPayload {
    SubmissionAccepted(OutputHealthRecordStamp),
    SubmissionDropped {
        stamp: OutputHealthRecordStamp,
        cause: DiagnosticDropCause,
    },
    SubmissionClosed {
        stamp: OutputHealthRecordStamp,
        cause: DiagnosticClosedCause,
    },
    DestinationResult(DiagnosticDestinationResult),
    ActivationFailure {
        output: LifecycleIdentity,
        source_stream: LifecycleIdentity,
        cause: OutputHealthActivationFailureCause,
        rollback: RollbackOutcome,
    },
    RuntimeFailure {
        output: LifecycleIdentity,
        source_stream: LifecycleIdentity,
        cause: OutputHealthRuntimeFailureCause,
        execution: ExecutionOutcome,
    },
    IncompleteShutdown {
        output: LifecycleIdentity,
        source_stream: LifecycleIdentity,
        unresolved_records: CheckedCount,
        owner_at_latch: DiagnosticOutputOwnerState,
    },
}

/// Coherent pre-sequence input to a future `OutputHealth` record operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutputHealthProposal {
    payload: OutputHealthProposalPayload,
}

/// Failure to construct one coherent `OutputHealth` proposal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputHealthProposalError {
    /// A post-activation cause conflicts with execution evidence.
    RuntimeExecutionMismatch {
        /// Runtime failure cause being validated.
        cause: OutputHealthRuntimeFailureCause,
        /// Rejected execution evidence.
        execution: ExecutionOutcome,
    },
    /// A completed shutdown report cannot propose incomplete shutdown.
    ShutdownComplete,
}

/// Nonzero record and charged-byte maxima for one health snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutputHealthSnapshotLimits {
    records: CheckedCount,
    bytes: CheckedByteCount,
}

/// Hard work, storage, snapshot, and import bounds for `OutputHealth`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutputHealthLimits {
    stream: LifecycleIdentity,
    record_work: BoundDescriptor,
    temporary_bytes: BoundDescriptor,
    fixed_resident_bytes: BoundDescriptor,
    anomaly_records: BoundDescriptor,
    anomaly_bytes: BoundDescriptor,
    resident_bytes: BoundDescriptor,
    snapshot: OutputHealthSnapshotLimits,
    import_attempts: CheckedCount,
}

/// Immutable contract, ownership, and bounds for one `OutputHealth` partition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutputHealthDescriptor {
    contract: ContractSelection,
    stream: LifecycleIdentity,
    lifecycle_owner: LifecycleIdentity,
    limits: OutputHealthLimits,
}

/// Checked nonzero order assigned only to an accepted health anomaly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutputHealthSequence(u64);

/// Next assignable health sequence or an exhausted sequence domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputHealthSequencePosition {
    /// This sequence is next after the represented state cut.
    Next(OutputHealthSequence),
    /// The latest assigned sequence is [`u64::MAX`].
    Exhausted,
}

/// Latest assigned health sequence, including an empty sequence history.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutputHealthHighWater {
    latest: Option<OutputHealthSequence>,
}

/// Inclusive retained anomaly range, including an empty range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutputHealthRetainedRange {
    earliest: Option<OutputHealthSequence>,
    latest: Option<OutputHealthSequence>,
}

/// Immutable per-output authority scope for one write-only health lease.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutputHealthLeaseScope {
    stream: LifecycleIdentity,
    lifecycle_owner: LifecycleIdentity,
    output: LifecycleIdentity,
}

/// Pure relation between a proposal and one immutable lease scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputHealthProposalScope {
    /// Output and source Stream generations match exactly.
    Current,
    /// Output or source Stream names another incarnation.
    Invalid,
    /// An incarnation matches but its generation differs.
    Stale,
}

/// Successful disposition of one `OutputHealth` record operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputHealthAcceptance {
    /// Fixed counters changed without consuming anomaly capacity.
    CounterOnly,
    /// One complete anomaly committed under this sequence.
    AnomalyRetained(OutputHealthSequence),
}

/// Complete producer-visible result of one health record operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputHealthRecordOutcome {
    /// Counter-only or complete-anomaly recording committed atomically.
    Accepted(OutputHealthAcceptance),
    /// Non-waiting state authority was unavailable.
    Pressure,
    /// Proposal identities name another output or Stream incarnation.
    Invalid,
    /// A matching incarnation used another generation.
    Stale,
    /// Health is Closing or Closed.
    Closed,
    /// No later anomaly sequence can be assigned without wrapping.
    SequenceExhausted,
}

/// Complete retained health anomaly with its fixed contract charge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutputHealthAnomalyRecord {
    sequence: OutputHealthSequence,
    proposal: OutputHealthProposal,
    resident_bytes: CheckedByteCount,
}

/// Accepted `DiagnosticOutput` submission record and byte counters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutputHealthAcceptedCounters {
    records: SaturatingCounter,
    record_bytes: SaturatingCounter,
}

/// Fixed counters for every `DiagnosticOutput` drop cause.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutputHealthDropCounters {
    counters: [SaturatingCounter; 10],
}

/// Fixed counters for every `DiagnosticOutput` closed cause.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutputHealthClosedCounters {
    counters: [SaturatingCounter; 4],
}

/// Fixed counters for every Effect Certainty result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutputHealthEffectCounters {
    counters: [SaturatingCounter; 3],
}

/// Complete-anomaly record and resident-byte eviction counters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutputHealthEvictionCounters {
    records: SaturatingCounter,
    resident_bytes: SaturatingCounter,
}

/// Closed semantic label for a nonaccepted health record result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputHealthRecordResultKind {
    /// Non-waiting state authority was unavailable.
    Pressure,
    /// Proposal identity was invalid for the lease.
    Invalid,
    /// Proposal or lease generation was stale.
    Stale,
    /// Health was Closing or Closed.
    Closed,
    /// The anomaly sequence domain was exhausted.
    SequenceExhaustion,
}

/// Fixed counters for nonaccepted health record results.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutputHealthRecordResultCounters {
    counters: [SaturatingCounter; 5],
}

/// Closed semantic label for one future health import result counter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputHealthImportCounterKind {
    /// One complete import entry was admitted as an Event.
    Accepted,
    /// Event admission rejected the pending entry.
    Rejected,
    /// Event admission dropped the pending entry.
    Dropped,
    /// Event admission was closed for the pending entry.
    Closed,
    /// The fixed aggregate import-attempt bound was reached.
    AttemptLimit,
}

/// Fixed labels and counters reserved for future health import.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutputHealthImportCounters {
    counters: [SaturatingCounter; 5],
}

/// Exact 31-slot `OutputHealth` counter snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutputHealthCounters {
    accepted: OutputHealthAcceptedCounters,
    dropped: OutputHealthDropCounters,
    closed: OutputHealthClosedCounters,
    effect: OutputHealthEffectCounters,
    eviction: OutputHealthEvictionCounters,
    record_results: OutputHealthRecordResultCounters,
    import: OutputHealthImportCounters,
}

/// Caller-owned record and charged-byte limits for a health snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutputHealthSnapshotRequest {
    stream: LifecycleIdentity,
    charge_owner: LifecycleIdentity,
    anomaly_bytes: CheckedByteCount,
    maximum_records: CheckedCount,
    maximum_bytes: CheckedByteCount,
}

/// Exact copied complete-record and charged-byte counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutputHealthSnapshotStatistics {
    copied_records: CheckedCount,
    copied_bytes: CheckedByteCount,
}

/// Whether one snapshot copied its complete retained range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputHealthSnapshotCoverage {
    /// Every retained anomaly at the state cut was copied.
    Complete,
    /// Capacity ended before this complete anomaly could be copied.
    Partial {
        /// First retained health sequence not present in the snapshot.
        first_uncopied: OutputHealthSequence,
    },
}

/// Bounded health snapshot facts before the complete-record join.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutputHealthSnapshotMetadata {
    stream: LifecycleIdentity,
    charge_owner: LifecycleIdentity,
    anomaly_bytes: CheckedByteCount,
    high_water: OutputHealthHighWater,
    retained: OutputHealthRetainedRange,
    statistics: OutputHealthSnapshotStatistics,
    next: OutputHealthSequencePosition,
    coverage: OutputHealthSnapshotCoverage,
}

/// Borrowed complete health anomalies and their coherent snapshot facts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutputHealthSnapshot<'a> {
    metadata: OutputHealthSnapshotMetadata,
    counters: &'a OutputHealthCounters,
    records: &'a [OutputHealthAnomalyRecord],
}

/// Failure to construct or validate an `OutputHealth` contract value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputHealthError {
    /// A Foundation bound rejected the declaration or arithmetic.
    Bound(BoundError),
    /// Limits were prepared for another Event Stream generation.
    LimitsStreamMismatch,
    /// Snapshot maxima cannot cover a useful subset of retained capacity.
    IncoherentSnapshotLimits,
    /// The requested `OutputHealth` contract family is unsupported.
    UnsupportedContract,
    /// The requested `OutputHealth` contract version is incompatible.
    IncompatibleContract,
    /// Zero cannot identify an assigned health anomaly.
    UnassignedSequence,
    /// The health sequence domain cannot advance without wrapping.
    SequenceExhausted,
    /// The `DiagnosticOutput` source differs from the health Stream.
    SourceStreamMismatch,
    /// A counter-only proposal cannot form a retained anomaly.
    CounterOnlyProposal,
    /// Earliest retained health sequence follows latest.
    InvalidRetainedRange,
    /// Snapshot capacity cannot contain the declared complete records.
    SnapshotBoundExceeded,
    /// Snapshot scalar, range, position, or coverage facts disagree.
    InvalidSnapshotMetadata,
    /// Borrowed anomaly records disagree with snapshot metadata.
    InvalidSnapshotRecords,
}

/// Write-only port for one generation-bound `OutputHealth` scope.
pub trait OutputHealthLease {
    /// Returns the immutable health declaration.
    fn descriptor(&self) -> &OutputHealthDescriptor;

    /// Returns this lease's exact output and Stream scope.
    fn scope(&self) -> OutputHealthLeaseScope;

    /// Attempts one bounded local record operation.
    fn record(
        &self,
        proposal: OutputHealthProposal,
    ) -> OutputHealthRecordOutcome;
}

impl OutputHealthProposal {
    /// Proposes accepted-record and byte accounting before record movement.
    #[must_use]
    pub const fn submission_accepted(record: &DiagnosticOutputRecord) -> Self {
        Self {
            payload: OutputHealthProposalPayload::SubmissionAccepted(
                OutputHealthRecordStamp::from_record(record),
            ),
        }
    }

    /// Proposes one typed live-output drop from its returned record.
    #[must_use]
    pub const fn submission_dropped(
        record: &DiagnosticOutputRecord,
        cause: DiagnosticDropCause,
    ) -> Self {
        Self {
            payload: OutputHealthProposalPayload::SubmissionDropped {
                stamp: OutputHealthRecordStamp::from_record(record),
                cause,
            },
        }
    }

    /// Proposes one typed lifecycle close from its returned record.
    #[must_use]
    pub const fn submission_closed(
        record: &DiagnosticOutputRecord,
        cause: DiagnosticClosedCause,
    ) -> Self {
        Self {
            payload: OutputHealthProposalPayload::SubmissionClosed {
                stamp: OutputHealthRecordStamp::from_record(record),
                cause,
            },
        }
    }

    /// Proposes effect accounting and any non-success destination anomaly.
    #[must_use]
    pub const fn destination_result(
        destination: DiagnosticDestinationResult,
    ) -> Self {
        Self {
            payload: OutputHealthProposalPayload::DestinationResult(
                destination,
            ),
        }
    }

    /// Proposes one activation failure with its bounded rollback result.
    #[must_use]
    pub const fn activation_failure(
        descriptor: &DiagnosticOutputDescriptor,
        cause: OutputHealthActivationFailureCause,
        rollback: RollbackOutcome,
    ) -> Self {
        Self {
            payload: OutputHealthProposalPayload::ActivationFailure {
                output: descriptor.lifecycle(),
                source_stream: descriptor.source_stream(),
                cause,
                rollback,
            },
        }
    }

    /// Proposes one coherent failure after output activation.
    ///
    /// # Errors
    ///
    /// Returns [`OutputHealthProposalError::RuntimeExecutionMismatch`] when
    /// execution never started or unexpected termination remains nonterminal.
    pub const fn runtime_failure(
        descriptor: &DiagnosticOutputDescriptor,
        cause: OutputHealthRuntimeFailureCause,
        execution: ExecutionOutcome,
    ) -> Result<Self, OutputHealthProposalError> {
        if !runtime_execution_matches(cause, execution) {
            return Err(OutputHealthProposalError::RuntimeExecutionMismatch {
                cause,
                execution,
            });
        }
        Ok(Self {
            payload: OutputHealthProposalPayload::RuntimeFailure {
                output: descriptor.lifecycle(),
                source_stream: descriptor.source_stream(),
                cause,
                execution,
            },
        })
    }

    /// Proposes incomplete shutdown from one already coherent report.
    ///
    /// # Errors
    ///
    /// Returns [`OutputHealthProposalError::ShutdownComplete`] for a report
    /// whose derived outcome is Closed.
    pub const fn incomplete_shutdown(
        report: DiagnosticShutdownReportRef<'_>,
    ) -> Result<Self, OutputHealthProposalError> {
        match report.outcome() {
            ShutdownOutcome::IncompleteShutdown => {}
            ShutdownOutcome::Closed => {
                return Err(OutputHealthProposalError::ShutdownComplete);
            }
        }
        Ok(Self {
            payload: OutputHealthProposalPayload::IncompleteShutdown {
                output: report.output(),
                source_stream: report.source_stream(),
                unresolved_records: report.statistics().unresolved_records(),
                owner_at_latch: report.owner_at_latch(),
            },
        })
    }

    /// Returns the proposal's closed semantic category.
    #[must_use]
    pub const fn kind(self) -> OutputHealthProposalKind {
        self.payload.kind()
    }

    /// Returns the target Diagnostic Output lifecycle.
    #[must_use]
    pub const fn output(self) -> LifecycleIdentity {
        self.payload.output()
    }

    /// Returns the source Event Stream lifecycle.
    #[must_use]
    pub const fn source_stream(self) -> LifecycleIdentity {
        self.payload.source_stream()
    }

    /// Reports whether successful recording would retain an anomaly.
    #[must_use]
    pub const fn is_anomaly_candidate(self) -> bool {
        match self.payload {
            OutputHealthProposalPayload::SubmissionAccepted(_)
            | OutputHealthProposalPayload::SubmissionDropped { .. }
            | OutputHealthProposalPayload::SubmissionClosed { .. } => false,
            OutputHealthProposalPayload::DestinationResult(destination) => {
                destination_is_anomaly(destination)
            }
            OutputHealthProposalPayload::ActivationFailure { .. }
            | OutputHealthProposalPayload::RuntimeFailure { .. }
            | OutputHealthProposalPayload::IncompleteShutdown { .. } => true,
        }
    }

    /// Returns scalar record evidence for a submission proposal.
    #[must_use]
    pub const fn record_stamp(self) -> Option<OutputHealthRecordStamp> {
        match self.payload {
            OutputHealthProposalPayload::SubmissionAccepted(stamp)
            | OutputHealthProposalPayload::SubmissionDropped {
                stamp, ..
            }
            | OutputHealthProposalPayload::SubmissionClosed { stamp, .. } => {
                Some(stamp)
            }
            OutputHealthProposalPayload::DestinationResult(_)
            | OutputHealthProposalPayload::ActivationFailure { .. }
            | OutputHealthProposalPayload::RuntimeFailure { .. }
            | OutputHealthProposalPayload::IncompleteShutdown { .. } => None,
        }
    }

    /// Returns the cause only for a dropped-submission proposal.
    #[must_use]
    pub const fn drop_cause(self) -> Option<DiagnosticDropCause> {
        match self.payload {
            OutputHealthProposalPayload::SubmissionDropped {
                cause, ..
            } => Some(cause),
            _ => None,
        }
    }

    /// Returns the cause only for a closed-submission proposal.
    #[must_use]
    pub const fn closed_cause(self) -> Option<DiagnosticClosedCause> {
        match self.payload {
            OutputHealthProposalPayload::SubmissionClosed { cause, .. } => {
                Some(cause)
            }
            _ => None,
        }
    }

    /// Returns evidence only for a destination-result proposal.
    #[must_use]
    pub const fn destination(self) -> Option<DiagnosticDestinationResult> {
        match self.payload {
            OutputHealthProposalPayload::DestinationResult(destination) => {
                Some(destination)
            }
            _ => None,
        }
    }

    /// Returns the cause only for an activation-failure proposal.
    #[must_use]
    pub const fn activation_cause(
        self,
    ) -> Option<OutputHealthActivationFailureCause> {
        match self.payload {
            OutputHealthProposalPayload::ActivationFailure {
                cause, ..
            } => Some(cause),
            _ => None,
        }
    }

    /// Returns rollback only for an activation-failure proposal.
    #[must_use]
    pub const fn rollback(self) -> Option<RollbackOutcome> {
        match self.payload {
            OutputHealthProposalPayload::ActivationFailure {
                rollback, ..
            } => Some(rollback),
            _ => None,
        }
    }

    /// Returns the cause only for a Runtime-failure proposal.
    #[must_use]
    pub const fn runtime_cause(
        self,
    ) -> Option<OutputHealthRuntimeFailureCause> {
        match self.payload {
            OutputHealthProposalPayload::RuntimeFailure { cause, .. } => {
                Some(cause)
            }
            _ => None,
        }
    }

    /// Returns execution only for a Runtime-failure proposal.
    #[must_use]
    pub const fn execution(self) -> Option<ExecutionOutcome> {
        match self.payload {
            OutputHealthProposalPayload::RuntimeFailure {
                execution, ..
            } => Some(execution),
            _ => None,
        }
    }

    /// Returns unresolved records only for incomplete shutdown.
    #[must_use]
    pub const fn unresolved_records(self) -> Option<CheckedCount> {
        match self.payload {
            OutputHealthProposalPayload::IncompleteShutdown {
                unresolved_records,
                ..
            } => Some(unresolved_records),
            _ => None,
        }
    }

    /// Returns output-owner state only for incomplete shutdown.
    #[must_use]
    pub const fn owner_at_latch(self) -> Option<DiagnosticOutputOwnerState> {
        match self.payload {
            OutputHealthProposalPayload::IncompleteShutdown {
                owner_at_latch,
                ..
            } => Some(owner_at_latch),
            _ => None,
        }
    }
}

impl OutputHealthProposalPayload {
    const fn kind(self) -> OutputHealthProposalKind {
        match self {
            Self::SubmissionAccepted(_) => {
                OutputHealthProposalKind::SubmissionAccepted
            }
            Self::SubmissionDropped { .. } => {
                OutputHealthProposalKind::SubmissionDropped
            }
            Self::SubmissionClosed { .. } => {
                OutputHealthProposalKind::SubmissionClosed
            }
            Self::DestinationResult(_) => {
                OutputHealthProposalKind::DestinationResult
            }
            Self::ActivationFailure { .. } => {
                OutputHealthProposalKind::ActivationFailure
            }
            Self::RuntimeFailure { .. } => {
                OutputHealthProposalKind::RuntimeFailure
            }
            Self::IncompleteShutdown { .. } => {
                OutputHealthProposalKind::IncompleteShutdown
            }
        }
    }

    const fn output(self) -> LifecycleIdentity {
        match self {
            Self::SubmissionAccepted(stamp)
            | Self::SubmissionDropped { stamp, .. }
            | Self::SubmissionClosed { stamp, .. } => stamp.output(),
            Self::DestinationResult(destination) => destination.output(),
            Self::ActivationFailure { output, .. }
            | Self::RuntimeFailure { output, .. }
            | Self::IncompleteShutdown { output, .. } => output,
        }
    }

    const fn source_stream(self) -> LifecycleIdentity {
        match self {
            Self::SubmissionAccepted(stamp)
            | Self::SubmissionDropped { stamp, .. }
            | Self::SubmissionClosed { stamp, .. } => stamp.source_stream(),
            Self::DestinationResult(destination) => destination.stream(),
            Self::ActivationFailure { source_stream, .. }
            | Self::RuntimeFailure { source_stream, .. }
            | Self::IncompleteShutdown { source_stream, .. } => source_stream,
        }
    }
}

impl fmt::Display for OutputHealthProposalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::RuntimeExecutionMismatch { .. } => {
                "OutputHealth Runtime failure execution is incoherent"
            }
            Self::ShutdownComplete => {
                "Diagnostic Output shutdown is already complete"
            }
        })
    }
}

impl std::error::Error for OutputHealthProposalError {}

const fn runtime_execution_matches(
    cause: OutputHealthRuntimeFailureCause,
    execution: ExecutionOutcome,
) -> bool {
    match execution {
        ExecutionOutcome::NeverStarted => false,
        ExecutionOutcome::OwnedNonterminal => {
            runtime_cause_allows_nonterminal(cause)
        }
        ExecutionOutcome::Stopped
        | ExecutionOutcome::ContainingProcessReaped => true,
    }
}

const fn runtime_cause_allows_nonterminal(
    cause: OutputHealthRuntimeFailureCause,
) -> bool {
    match cause {
        OutputHealthRuntimeFailureCause::DestinationUnavailable
        | OutputHealthRuntimeFailureCause::HealthUnavailable
        | OutputHealthRuntimeFailureCause::InternalFailure => true,
        OutputHealthRuntimeFailureCause::UnexpectedExecutionTermination => {
            false
        }
    }
}

const fn destination_is_anomaly(
    destination: DiagnosticDestinationResult,
) -> bool {
    match destination.cause() {
        DiagnosticDestinationCause::Success => false,
        DiagnosticDestinationCause::ClosingDiscard
        | DiagnosticDestinationCause::ReplacementDiscard
        | DiagnosticDestinationCause::Cancellation
        | DiagnosticDestinationCause::ShutdownDeadline
        | DiagnosticDestinationCause::WriteZero
        | DiagnosticDestinationCause::WriteFailure
        | DiagnosticDestinationCause::RetryExhaustion
        | DiagnosticDestinationCause::WriterContractViolation
        | DiagnosticDestinationCause::InternalFailureBeforeCall
        | DiagnosticDestinationCause::InternalFailureInCall
        | DiagnosticDestinationCause::SupervisorReaping => true,
    }
}
