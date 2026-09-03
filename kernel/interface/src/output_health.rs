//! Pre-sequence proposals for the Event Stream-owned `OutputHealth` partition.

use core::fmt;

use crate::cancellation::ExecutionOutcome;
use crate::diagnostic::{
    CheckedByteCount, CheckedCount, DiagnosticClosedCause,
    DiagnosticDestinationCause, DiagnosticDestinationResult,
    DiagnosticDropCause, DiagnosticOutputDescriptor,
    DiagnosticOutputOwnerState, DiagnosticOutputRecord,
    DiagnosticShutdownReportRef,
};
use crate::event::EventSequence;
use crate::lifecycle::{LifecycleIdentity, RollbackOutcome, ShutdownOutcome};

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
