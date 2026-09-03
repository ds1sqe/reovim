//! `OutputHealth` proposal values through the public contract.

use reovim_kernel_interface::bound::{CheckedByteCount, ZeroPolicy};
use reovim_kernel_interface::cancellation::ExecutionOutcome;
use reovim_kernel_interface::diagnostic::{
    DiagnosticClosedCause, DiagnosticDestinationCause,
    DiagnosticDestinationCut, DiagnosticDestinationResult, DiagnosticDropCause,
    DiagnosticOutput, DiagnosticOutputDescriptor, DiagnosticOutputOwnerState,
    DiagnosticOutputRecord, DiagnosticShutdownEntry,
    DiagnosticShutdownReportRef, DiagnosticSubmissionOutcome,
    DiagnosticSubmissionStatus,
};
use reovim_kernel_interface::effect::EffectCertainty;
use reovim_kernel_interface::lifecycle::{RollbackOutcome, WorkDisposition};
use reovim_kernel_interface::output_health::{
    OutputHealthActivationFailureCause, OutputHealthProposal,
    OutputHealthProposalError, OutputHealthProposalKind,
    OutputHealthRecordStamp, OutputHealthRuntimeFailureCause,
};

use crate::{error_traits, traits};

const DROP_CAUSES: [DiagnosticDropCause; 10] = [
    DiagnosticDropCause::InvalidRecord,
    DiagnosticDropCause::BoundExceeded,
    DiagnosticDropCause::NotReady,
    DiagnosticDropCause::SynchronizationPressure,
    DiagnosticDropCause::RecordCapacity,
    DiagnosticDropCause::ByteCapacity,
    DiagnosticDropCause::ChargeOverflow,
    DiagnosticDropCause::DestinationUnavailable,
    DiagnosticDropCause::HealthUnavailable,
    DiagnosticDropCause::InternalFailure,
];

const CLOSED_CAUSES: [DiagnosticClosedCause; 4] = [
    DiagnosticClosedCause::Closing,
    DiagnosticClosedCause::Closed,
    DiagnosticClosedCause::Replaced,
    DiagnosticClosedCause::IncompleteShutdown,
];

const DESTINATION_CAUSES: [DiagnosticDestinationCause; 12] = [
    DiagnosticDestinationCause::Success,
    DiagnosticDestinationCause::ClosingDiscard,
    DiagnosticDestinationCause::ReplacementDiscard,
    DiagnosticDestinationCause::Cancellation,
    DiagnosticDestinationCause::ShutdownDeadline,
    DiagnosticDestinationCause::WriteZero,
    DiagnosticDestinationCause::WriteFailure,
    DiagnosticDestinationCause::RetryExhaustion,
    DiagnosticDestinationCause::WriterContractViolation,
    DiagnosticDestinationCause::InternalFailureBeforeCall,
    DiagnosticDestinationCause::InternalFailureInCall,
    DiagnosticDestinationCause::SupervisorReaping,
];

const ACTIVATION_CAUSES: [OutputHealthActivationFailureCause; 4] = [
    OutputHealthActivationFailureCause::DestinationUnavailable,
    OutputHealthActivationFailureCause::HealthUnavailable,
    OutputHealthActivationFailureCause::ExecutionStartFailure,
    OutputHealthActivationFailureCause::InternalFailure,
];

const RUNTIME_CAUSES: [OutputHealthRuntimeFailureCause; 4] = [
    OutputHealthRuntimeFailureCause::DestinationUnavailable,
    OutputHealthRuntimeFailureCause::HealthUnavailable,
    OutputHealthRuntimeFailureCause::UnexpectedExecutionTermination,
    OutputHealthRuntimeFailureCause::InternalFailure,
];

const EXECUTIONS: [ExecutionOutcome; 4] = [
    ExecutionOutcome::NeverStarted,
    ExecutionOutcome::OwnedNonterminal,
    ExecutionOutcome::Stopped,
    ExecutionOutcome::ContainingProcessReaped,
];

const _: fn(&DiagnosticOutputRecord) -> OutputHealthRecordStamp =
    OutputHealthRecordStamp::from_record;
const _: fn(&DiagnosticOutputRecord) -> OutputHealthProposal =
    OutputHealthProposal::submission_accepted;
const _: fn(
    &DiagnosticOutputRecord,
    DiagnosticDropCause,
) -> OutputHealthProposal = OutputHealthProposal::submission_dropped;
const _: fn(
    &DiagnosticOutputRecord,
    DiagnosticClosedCause,
) -> OutputHealthProposal = OutputHealthProposal::submission_closed;
const _: fn(DiagnosticDestinationResult) -> OutputHealthProposal =
    OutputHealthProposal::destination_result;
const _: fn(
    &DiagnosticOutputDescriptor,
    OutputHealthActivationFailureCause,
    RollbackOutcome,
) -> OutputHealthProposal = OutputHealthProposal::activation_failure;
const _: fn(
    &DiagnosticOutputDescriptor,
    OutputHealthRuntimeFailureCause,
    ExecutionOutcome,
) -> Result<OutputHealthProposal, OutputHealthProposalError> =
    OutputHealthProposal::runtime_failure;
const _: for<'a> fn(
    DiagnosticShutdownReportRef<'a>,
)
    -> Result<OutputHealthProposal, OutputHealthProposalError> =
    OutputHealthProposal::incomplete_shutdown;

pub const fn const_cases() -> bool {
    crate::derive_witness::<
        ::reovim_kernel_interface::output_health::OutputHealthRecordStamp,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::output_health::OutputHealthProposalKind,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::output_health::
            OutputHealthActivationFailureCause,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::output_health::
            OutputHealthRuntimeFailureCause,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::output_health::OutputHealthProposal,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::output_health::OutputHealthProposalError,
    >()
        && proposal_kind_variants()
        && activation_cause_variants()
        && runtime_cause_variants()
}

pub fn exercise() -> bool {
    record_stamp_case()
        && descriptor_borrow_case()
        && submission_accepted_case()
        && DROP_CAUSES.into_iter().all(submission_dropped_case)
        && CLOSED_CAUSES.into_iter().all(submission_closed_case)
        && DESTINATION_CAUSES.into_iter().all(destination_result_case)
        && activation_relation()
        && runtime_relation()
        && incomplete_shutdown_cases()
        && error_cases()
}

const fn proposal_kind_variants() -> bool {
    proposal_kind_code(OutputHealthProposalKind::SubmissionAccepted) == 0
        && proposal_kind_code(OutputHealthProposalKind::SubmissionDropped) == 1
        && proposal_kind_code(OutputHealthProposalKind::SubmissionClosed) == 2
        && proposal_kind_code(OutputHealthProposalKind::DestinationResult) == 3
        && proposal_kind_code(OutputHealthProposalKind::ActivationFailure) == 4
        && proposal_kind_code(OutputHealthProposalKind::RuntimeFailure) == 5
        && proposal_kind_code(OutputHealthProposalKind::IncompleteShutdown) == 6
}

const fn proposal_kind_code(value: OutputHealthProposalKind) -> u8 {
    match value {
        OutputHealthProposalKind::SubmissionAccepted => 0,
        OutputHealthProposalKind::SubmissionDropped => 1,
        OutputHealthProposalKind::SubmissionClosed => 2,
        OutputHealthProposalKind::DestinationResult => 3,
        OutputHealthProposalKind::ActivationFailure => 4,
        OutputHealthProposalKind::RuntimeFailure => 5,
        OutputHealthProposalKind::IncompleteShutdown => 6,
    }
}

const fn activation_cause_variants() -> bool {
    activation_cause_code(
        OutputHealthActivationFailureCause::DestinationUnavailable,
    ) == 0
        && activation_cause_code(
            OutputHealthActivationFailureCause::HealthUnavailable,
        ) == 1
        && activation_cause_code(
            OutputHealthActivationFailureCause::ExecutionStartFailure,
        ) == 2
        && activation_cause_code(
            OutputHealthActivationFailureCause::InternalFailure,
        ) == 3
}

const fn activation_cause_code(
    value: OutputHealthActivationFailureCause,
) -> u8 {
    match value {
        OutputHealthActivationFailureCause::DestinationUnavailable => 0,
        OutputHealthActivationFailureCause::HealthUnavailable => 1,
        OutputHealthActivationFailureCause::ExecutionStartFailure => 2,
        OutputHealthActivationFailureCause::InternalFailure => 3,
    }
}

const fn runtime_cause_variants() -> bool {
    runtime_cause_code(OutputHealthRuntimeFailureCause::DestinationUnavailable)
        == 0
        && runtime_cause_code(
            OutputHealthRuntimeFailureCause::HealthUnavailable,
        ) == 1
        && runtime_cause_code(
            OutputHealthRuntimeFailureCause::UnexpectedExecutionTermination,
        ) == 2
        && runtime_cause_code(OutputHealthRuntimeFailureCause::InternalFailure)
            == 3
}

const fn runtime_cause_code(value: OutputHealthRuntimeFailureCause) -> u8 {
    match value {
        OutputHealthRuntimeFailureCause::DestinationUnavailable => 0,
        OutputHealthRuntimeFailureCause::HealthUnavailable => 1,
        OutputHealthRuntimeFailureCause::UnexpectedExecutionTermination => 2,
        OutputHealthRuntimeFailureCause::InternalFailure => 3,
    }
}

fn record_stamp_case() -> bool {
    let Some(descriptor) = crate::diagnostic::make_descriptor() else {
        return false;
    };
    let Some(record) = crate::diagnostic::make_record(&descriptor) else {
        return false;
    };
    let expected_bytes = record.size();
    let expected_sequence = record.sequence();
    let stamp = OutputHealthRecordStamp::from_record(&record);
    let output = record.output();
    let source_stream = record.stream();
    drop(record);
    traits(stamp)
        && stamp.output().same_generation(output)
        && stamp.source_stream().same_generation(source_stream)
        && stamp.sequence() == expected_sequence
        && stamp.record_bytes() == expected_bytes
}

#[derive(Clone, Copy)]
enum ScriptedDisposition {
    Accepted,
    Dropped(DiagnosticDropCause),
    Closed(DiagnosticClosedCause),
}

struct ScriptedOutput {
    descriptor: DiagnosticOutputDescriptor,
    disposition: ScriptedDisposition,
}

impl DiagnosticOutput for ScriptedOutput {
    fn descriptor(&self) -> &DiagnosticOutputDescriptor {
        &self.descriptor
    }

    fn submit(
        &self,
        record: DiagnosticOutputRecord,
    ) -> DiagnosticSubmissionOutcome {
        match self.disposition {
            ScriptedDisposition::Accepted => {
                DiagnosticSubmissionOutcome::accepted()
            }
            ScriptedDisposition::Dropped(cause) => {
                DiagnosticSubmissionOutcome::dropped(cause, record)
            }
            ScriptedDisposition::Closed(cause) => {
                DiagnosticSubmissionOutcome::closed(cause, record)
            }
        }
    }
}

fn submission_accepted_case() -> bool {
    let Some(descriptor) = crate::diagnostic::make_descriptor() else {
        return false;
    };
    let Some(record) = crate::diagnostic::make_record(&descriptor) else {
        return false;
    };
    let proposal = OutputHealthProposal::submission_accepted(&record);
    let stamp_matches = proposal
        .record_stamp()
        .is_some_and(|stamp| stamp_matches_record(stamp, &record));
    let output = ScriptedOutput {
        descriptor: descriptor.clone(),
        disposition: ScriptedDisposition::Accepted,
    };
    let outcome = output.submit(record);
    stamp_matches
        && outcome.status() == DiagnosticSubmissionStatus::Accepted
        && proposal_matches(
            proposal,
            &descriptor,
            OutputHealthProposalKind::SubmissionAccepted,
            false,
        )
        && proposal.record_stamp().is_some()
        && proposal.drop_cause().is_none()
        && proposal.closed_cause().is_none()
        && submission_only_accessors(proposal)
}

fn submission_dropped_case(cause: DiagnosticDropCause) -> bool {
    let Some(descriptor) = crate::diagnostic::make_descriptor() else {
        return false;
    };
    let Some(record) = crate::diagnostic::make_record(&descriptor) else {
        return false;
    };
    let output = ScriptedOutput {
        descriptor: descriptor.clone(),
        disposition: ScriptedDisposition::Dropped(cause),
    };
    let outcome = output.submit(record);
    let (proposal, stamp_matches) = match &outcome {
        DiagnosticSubmissionOutcome::Dropped { cause: actual, record }
            if *actual == cause =>
        {
            let value =
                OutputHealthProposal::submission_dropped(record, *actual);
            let matches = value
                .record_stamp()
                .is_some_and(|stamp| stamp_matches_record(stamp, record));
            (value, matches)
        }
        DiagnosticSubmissionOutcome::Accepted
        | DiagnosticSubmissionOutcome::Dropped { .. }
        | DiagnosticSubmissionOutcome::Closed { .. } => return false,
    };
    let returned = outcome.into_returned_record();
    returned.is_some()
        && stamp_matches
        && proposal_matches(
            proposal,
            &descriptor,
            OutputHealthProposalKind::SubmissionDropped,
            false,
        )
        && proposal.record_stamp().is_some()
        && proposal.drop_cause() == Some(cause)
        && proposal.closed_cause().is_none()
        && submission_only_accessors(proposal)
}

fn submission_closed_case(cause: DiagnosticClosedCause) -> bool {
    let Some(descriptor) = crate::diagnostic::make_descriptor() else {
        return false;
    };
    let Some(record) = crate::diagnostic::make_record(&descriptor) else {
        return false;
    };
    let output = ScriptedOutput {
        descriptor: descriptor.clone(),
        disposition: ScriptedDisposition::Closed(cause),
    };
    let outcome = output.submit(record);
    let (proposal, stamp_matches) = match &outcome {
        DiagnosticSubmissionOutcome::Closed { cause: actual, record }
            if *actual == cause =>
        {
            let value =
                OutputHealthProposal::submission_closed(record, *actual);
            let matches = value
                .record_stamp()
                .is_some_and(|stamp| stamp_matches_record(stamp, record));
            (value, matches)
        }
        DiagnosticSubmissionOutcome::Accepted
        | DiagnosticSubmissionOutcome::Dropped { .. }
        | DiagnosticSubmissionOutcome::Closed { .. } => return false,
    };
    let returned = outcome.into_returned_record();
    returned.is_some()
        && stamp_matches
        && proposal_matches(
            proposal,
            &descriptor,
            OutputHealthProposalKind::SubmissionClosed,
            false,
        )
        && proposal.record_stamp().is_some()
        && proposal.drop_cause().is_none()
        && proposal.closed_cause() == Some(cause)
        && submission_only_accessors(proposal)
}

const fn submission_only_accessors(proposal: OutputHealthProposal) -> bool {
    proposal.destination().is_none()
        && proposal.activation_cause().is_none()
        && proposal.rollback().is_none()
        && proposal.runtime_cause().is_none()
        && proposal.execution().is_none()
        && proposal.unresolved_records().is_none()
        && proposal.owner_at_latch().is_none()
}

fn stamp_matches_record(
    stamp: OutputHealthRecordStamp,
    record: &DiagnosticOutputRecord,
) -> bool {
    let stamp_bytes = stamp.record_bytes();
    let record_bytes = record.size();
    stamp.output().same_generation(record.output())
        && stamp.source_stream().same_generation(record.stream())
        && stamp.sequence() == record.sequence()
        && stamp_bytes == record_bytes
}

fn descriptor_borrow_case() -> bool {
    let Some((activation, runtime, output, stream)) =
        proposals_without_descriptor()
    else {
        return false;
    };
    activation.output().same_generation(output)
        && activation.source_stream().same_generation(stream)
        && runtime.output().same_generation(output)
        && runtime.source_stream().same_generation(stream)
}

fn proposals_without_descriptor() -> Option<(
    OutputHealthProposal,
    OutputHealthProposal,
    reovim_kernel_interface::lifecycle::LifecycleIdentity,
    reovim_kernel_interface::lifecycle::LifecycleIdentity,
)> {
    let descriptor = crate::diagnostic::make_descriptor()?;
    let activation = OutputHealthProposal::activation_failure(
        &descriptor,
        OutputHealthActivationFailureCause::InternalFailure,
        RollbackOutcome::Reverted,
    );
    let runtime = OutputHealthProposal::runtime_failure(
        &descriptor,
        OutputHealthRuntimeFailureCause::InternalFailure,
        ExecutionOutcome::Stopped,
    )
    .ok()?;
    Some((
        activation,
        runtime,
        descriptor.lifecycle(),
        descriptor.source_stream(),
    ))
}

fn destination_result_case(cause: DiagnosticDestinationCause) -> bool {
    let Some(descriptor) = crate::diagnostic::make_descriptor() else {
        return false;
    };
    let Some(record) = crate::diagnostic::make_record(&descriptor) else {
        return false;
    };
    let Some(destination) = make_destination(&record, cause) else {
        return false;
    };
    let proposal = OutputHealthProposal::destination_result(destination);
    drop(record);
    proposal_matches(
        proposal,
        &descriptor,
        OutputHealthProposalKind::DestinationResult,
        cause != DiagnosticDestinationCause::Success,
    ) && proposal.record_stamp().is_none()
        && proposal.destination() == Some(destination)
        && proposal.drop_cause().is_none()
        && proposal.closed_cause().is_none()
        && lifecycle_accessors_are_empty(proposal)
}

fn make_destination(
    record: &DiagnosticOutputRecord,
    cause: DiagnosticDestinationCause,
) -> Option<DiagnosticDestinationResult> {
    let (prefix, certainty, cut) = destination_fields(record, cause)?;
    DiagnosticDestinationResult::new(record, prefix, certainty, cause, cut).ok()
}

fn destination_fields(
    record: &DiagnosticOutputRecord,
    cause: DiagnosticDestinationCause,
) -> Option<(CheckedByteCount, EffectCertainty, DiagnosticDestinationCut)> {
    let zero = CheckedByteCount::new(0, ZeroPolicy::Allow).ok()?;
    let fields = match cause {
        DiagnosticDestinationCause::Success => (
            record.size(),
            EffectCertainty::Applied,
            DiagnosticDestinationCut::Returned,
        ),
        DiagnosticDestinationCause::ClosingDiscard
        | DiagnosticDestinationCause::ReplacementDiscard
        | DiagnosticDestinationCause::Cancellation
        | DiagnosticDestinationCause::ShutdownDeadline
        | DiagnosticDestinationCause::InternalFailureBeforeCall
        | DiagnosticDestinationCause::SupervisorReaping => (
            zero,
            EffectCertainty::DefinitelyNotApplied,
            DiagnosticDestinationCut::BeforeCall,
        ),
        DiagnosticDestinationCause::WriteZero
        | DiagnosticDestinationCause::WriteFailure
        | DiagnosticDestinationCause::RetryExhaustion => (
            zero,
            EffectCertainty::DefinitelyNotApplied,
            DiagnosticDestinationCut::Returned,
        ),
        DiagnosticDestinationCause::WriterContractViolation => (
            zero,
            EffectCertainty::OutcomeUnknown,
            DiagnosticDestinationCut::Returned,
        ),
        DiagnosticDestinationCause::InternalFailureInCall => (
            zero,
            EffectCertainty::OutcomeUnknown,
            DiagnosticDestinationCut::InCall,
        ),
    };
    Some(fields)
}

fn activation_relation() -> bool {
    let Some(descriptor) = crate::diagnostic::make_descriptor() else {
        return false;
    };
    ACTIVATION_CAUSES.into_iter().all(|cause| {
        activation_case(&descriptor, cause, RollbackOutcome::Reverted)
            && activation_case(
                &descriptor,
                cause,
                RollbackOutcome::IncompleteShutdown,
            )
    })
}

fn activation_case(
    descriptor: &DiagnosticOutputDescriptor,
    cause: OutputHealthActivationFailureCause,
    rollback: RollbackOutcome,
) -> bool {
    let proposal =
        OutputHealthProposal::activation_failure(descriptor, cause, rollback);
    proposal_matches(
        proposal,
        descriptor,
        OutputHealthProposalKind::ActivationFailure,
        true,
    ) && proposal.activation_cause() == Some(cause)
        && proposal.rollback() == Some(rollback)
        && proposal.record_stamp().is_none()
        && proposal.destination().is_none()
        && proposal.runtime_cause().is_none()
        && proposal.execution().is_none()
        && proposal.unresolved_records().is_none()
        && proposal.owner_at_latch().is_none()
        && submission_cause_accessors_are_empty(proposal)
}

fn runtime_relation() -> bool {
    let Some(descriptor) = crate::diagnostic::make_descriptor() else {
        return false;
    };
    RUNTIME_CAUSES.into_iter().all(|cause| {
        EXECUTIONS
            .into_iter()
            .all(|execution| runtime_case(&descriptor, cause, execution))
    })
}

fn runtime_case(
    descriptor: &DiagnosticOutputDescriptor,
    cause: OutputHealthRuntimeFailureCause,
    execution: ExecutionOutcome,
) -> bool {
    let result =
        OutputHealthProposal::runtime_failure(descriptor, cause, execution);
    if runtime_pair_is_valid(cause, execution) {
        return result.is_ok_and(|proposal| {
            runtime_proposal_matches(proposal, descriptor, cause, execution)
        });
    }
    match result {
        Err(OutputHealthProposalError::RuntimeExecutionMismatch {
            cause: actual_cause,
            execution: actual_execution,
        }) => actual_cause == cause && actual_execution == execution,
        Ok(_) | Err(OutputHealthProposalError::ShutdownComplete) => false,
    }
}

const fn runtime_pair_is_valid(
    cause: OutputHealthRuntimeFailureCause,
    execution: ExecutionOutcome,
) -> bool {
    match execution {
        ExecutionOutcome::NeverStarted => false,
        ExecutionOutcome::OwnedNonterminal => match cause {
            OutputHealthRuntimeFailureCause::DestinationUnavailable
            | OutputHealthRuntimeFailureCause::HealthUnavailable
            | OutputHealthRuntimeFailureCause::InternalFailure => true,
            OutputHealthRuntimeFailureCause::UnexpectedExecutionTermination => {
                false
            }
        },
        ExecutionOutcome::Stopped
        | ExecutionOutcome::ContainingProcessReaped => true,
    }
}

fn runtime_proposal_matches(
    proposal: OutputHealthProposal,
    descriptor: &DiagnosticOutputDescriptor,
    cause: OutputHealthRuntimeFailureCause,
    execution: ExecutionOutcome,
) -> bool {
    proposal_matches(
        proposal,
        descriptor,
        OutputHealthProposalKind::RuntimeFailure,
        true,
    ) && proposal.runtime_cause() == Some(cause)
        && proposal.execution() == Some(execution)
        && proposal.record_stamp().is_none()
        && proposal.destination().is_none()
        && proposal.activation_cause().is_none()
        && proposal.rollback().is_none()
        && proposal.unresolved_records().is_none()
        && proposal.owner_at_latch().is_none()
        && submission_cause_accessors_are_empty(proposal)
}

fn incomplete_shutdown_cases() -> bool {
    unresolved_shutdown_case()
        && owner_only_shutdown_case(DiagnosticOutputOwnerState::LiveOwned)
        && owner_only_shutdown_case(DiagnosticOutputOwnerState::ExitedOwned)
        && complete_shutdown_rejected()
}

fn unresolved_shutdown_case() -> bool {
    let Some(descriptor) = crate::diagnostic::make_descriptor() else {
        return false;
    };
    let Some(record) = crate::diagnostic::make_record(&descriptor) else {
        return false;
    };
    let Some(destination) =
        make_destination(&record, DiagnosticDestinationCause::Success)
    else {
        return false;
    };
    let Ok(entry) = DiagnosticShutdownEntry::new(
        destination,
        WorkDisposition::Unresolved,
        ExecutionOutcome::OwnedNonterminal,
    ) else {
        return false;
    };
    let proposal = {
        let entries = [entry];
        let Ok(report) = DiagnosticShutdownReportRef::new(
            &descriptor,
            DiagnosticOutputOwnerState::LiveOwned,
            &entries,
        ) else {
            return false;
        };
        let Ok(value) = OutputHealthProposal::incomplete_shutdown(report)
        else {
            return false;
        };
        value
    };
    drop(record);
    incomplete_proposal_matches(
        proposal,
        &descriptor,
        1,
        DiagnosticOutputOwnerState::LiveOwned,
    )
}

fn owner_only_shutdown_case(owner: DiagnosticOutputOwnerState) -> bool {
    let Some(descriptor) = crate::diagnostic::make_descriptor() else {
        return false;
    };
    let proposal = {
        let Ok(report) =
            DiagnosticShutdownReportRef::new(&descriptor, owner, &[])
        else {
            return false;
        };
        let Ok(value) = OutputHealthProposal::incomplete_shutdown(report)
        else {
            return false;
        };
        value
    };
    incomplete_proposal_matches(proposal, &descriptor, 0, owner)
}

fn complete_shutdown_rejected() -> bool {
    let Some(descriptor) = crate::diagnostic::make_descriptor() else {
        return false;
    };
    let Ok(report) = DiagnosticShutdownReportRef::new(
        &descriptor,
        DiagnosticOutputOwnerState::NoOwnedExecution,
        &[],
    ) else {
        return false;
    };
    OutputHealthProposal::incomplete_shutdown(report)
        == Err(OutputHealthProposalError::ShutdownComplete)
}

fn incomplete_proposal_matches(
    proposal: OutputHealthProposal,
    descriptor: &DiagnosticOutputDescriptor,
    unresolved: u64,
    owner: DiagnosticOutputOwnerState,
) -> bool {
    proposal_matches(
        proposal,
        descriptor,
        OutputHealthProposalKind::IncompleteShutdown,
        true,
    ) && proposal
        .unresolved_records()
        .is_some_and(|value| value.value() == unresolved)
        && proposal.owner_at_latch() == Some(owner)
        && proposal.record_stamp().is_none()
        && proposal.destination().is_none()
        && proposal.activation_cause().is_none()
        && proposal.rollback().is_none()
        && proposal.runtime_cause().is_none()
        && proposal.execution().is_none()
        && submission_cause_accessors_are_empty(proposal)
}

fn proposal_matches(
    proposal: OutputHealthProposal,
    descriptor: &DiagnosticOutputDescriptor,
    kind: OutputHealthProposalKind,
    anomaly: bool,
) -> bool {
    traits(proposal)
        && proposal.kind() == kind
        && proposal.output().same_generation(descriptor.lifecycle())
        && proposal.source_stream().same_generation(descriptor.source_stream())
        && proposal.is_anomaly_candidate() == anomaly
}

const fn lifecycle_accessors_are_empty(proposal: OutputHealthProposal) -> bool {
    proposal.activation_cause().is_none()
        && proposal.rollback().is_none()
        && proposal.runtime_cause().is_none()
        && proposal.execution().is_none()
        && proposal.unresolved_records().is_none()
        && proposal.owner_at_latch().is_none()
}

const fn submission_cause_accessors_are_empty(
    proposal: OutputHealthProposal,
) -> bool {
    proposal.drop_cause().is_none() && proposal.closed_cause().is_none()
}

fn error_cases() -> bool {
    let runtime = OutputHealthProposalError::RuntimeExecutionMismatch {
        cause: OutputHealthRuntimeFailureCause::InternalFailure,
        execution: ExecutionOutcome::NeverStarted,
    };
    let shutdown = OutputHealthProposalError::ShutdownComplete;
    traits(runtime)
        && traits(shutdown)
        && error_traits(&runtime)
        && error_traits(&shutdown)
        && runtime.to_string()
            == "OutputHealth Runtime failure execution is incoherent"
        && shutdown.to_string()
            == "Diagnostic Output shutdown is already complete"
}
