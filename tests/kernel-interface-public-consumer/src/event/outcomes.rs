use reovim_kernel_interface::clock::ClockError;
use reovim_kernel_interface::event::{
    Actionability, BootstrapHealth, BoundError, CursorPosition,
    EventAdmissionOutcome, EventAdmissionScope, EventClosedCause,
    EventDropCause, EventError, EventRejectCause, EventSequence,
    RemediationCode, Severity, SnapshotCoverage, StartupComponentCounts,
    StartupEventDescriptor, StartupEventLimits, StartupProposal,
};
use reovim_kernel_interface::lifecycle::{
    Generation, LifecycleIdentity, ReadinessOutcome,
};

use super::{
    CORRELATION, PRODUCER, lifecycle_owner, make_descriptor, stream_identity,
};

struct FixedAdmission {
    descriptor: StartupEventDescriptor,
    scope: EventAdmissionScope,
    outcome: EventAdmissionOutcome,
}

impl ::reovim_kernel_interface::event::StartupEventAdmission
    for FixedAdmission
{
    fn descriptor(&self) -> &StartupEventDescriptor {
        &self.descriptor
    }

    fn scope(&self) -> EventAdmissionScope {
        self.scope
    }

    fn admit(&self, proposal: StartupProposal) -> EventAdmissionOutcome {
        if let Err(cause) = proposal.validate_for(&self.descriptor) {
            return EventAdmissionOutcome::Rejected(cause);
        }
        self.outcome
    }
}

pub fn exercise() -> bool {
    admission_cases()
        && scalar_variants()
        && rejection_variants()
        && drop_variants()
        && closed_variants()
        && admission_outcome_variants()
        && error_variants()
}

fn admission_cases() -> bool {
    let Some(descriptor) = make_descriptor() else {
        return false;
    };
    let Ok(scope) = EventAdmissionScope::new(
        &descriptor,
        lifecycle_owner(),
        PRODUCER,
        CORRELATION,
    ) else {
        return false;
    };
    let Ok(counts) = StartupComponentCounts::new(3, 0, 0) else {
        return false;
    };
    let Ok(proposal) = super::descriptor::make_proposal(
        &descriptor,
        ReadinessOutcome::Ready,
        BootstrapHealth::Clear,
        counts,
    ) else {
        return false;
    };
    let admission = FixedAdmission {
        descriptor: descriptor.clone(),
        scope,
        outcome: EventAdmissionOutcome::Accepted(EventSequence::first()),
    };
    let outcome =
        ::reovim_kernel_interface::event::StartupEventAdmission::admit(
            &admission, proposal,
        );

    ::reovim_kernel_interface::event::StartupEventAdmission::descriptor(
        &admission,
    ) == &descriptor
        && ::reovim_kernel_interface::event::StartupEventAdmission::scope(
            &admission,
        ) == scope
        && accepted_sequence_value(outcome) == 1
        && scope.stream().same_generation(stream_identity())
        && scope.lifecycle_owner().same_generation(lifecycle_owner())
        && scope.producer() == PRODUCER
        && scope.correlation() == CORRELATION
        && invalid_scope_case(&descriptor)
}

const fn accepted_sequence_value(outcome: EventAdmissionOutcome) -> u64 {
    match outcome.accepted_sequence() {
        Some(sequence) => sequence.value(),
        None => 0,
    }
}

const fn invalid_scope_case(descriptor: &StartupEventDescriptor) -> bool {
    let owner = lifecycle_owner();
    let stale = LifecycleIdentity::new(
        owner.instance(),
        owner.incarnation(),
        Generation::new(99),
    );
    match EventAdmissionScope::new(descriptor, stale, PRODUCER, CORRELATION) {
        Err(error) => event_error_code(error) == 7,
        Ok(_) => false,
    }
}

fn scalar_variants() -> bool {
    let Some(actionability) = operator_actionability() else {
        return false;
    };
    super::super::traits(BootstrapHealth::Clear)
        && super::super::traits(BootstrapHealth::Imported)
        && super::super::traits(BootstrapHealth::Pending)
        && super::super::traits(Severity::Info)
        && super::super::traits(Severity::Warning)
        && super::super::traits(Severity::Error)
        && super::super::traits(RemediationCode::DiagnosticPathDegraded)
        && super::super::traits(RemediationCode::ActivationFailed)
        && super::super::traits(Actionability::None)
        && super::super::traits(actionability)
        && super::super::traits(CursorPosition::Next(EventSequence::first()))
        && super::super::traits(CursorPosition::Exhausted)
        && super::super::traits(SnapshotCoverage::Complete)
        && super::super::traits(SnapshotCoverage::Partial {
            first_uncopied: EventSequence::first(),
        })
}

fn operator_actionability() -> Option<Actionability> {
    let descriptor = make_descriptor()?;
    let counts = StartupComponentCounts::new(1, 1, 0).ok()?;
    let proposal = super::descriptor::make_proposal(
        &descriptor,
        ReadinessOutcome::DiagnosticsDegraded,
        BootstrapHealth::Imported,
        counts,
    )
    .ok()?;
    Some(proposal.actionability())
}

fn rejection_variants() -> bool {
    super::super::traits(EventRejectCause::StreamNotReady)
        && super::super::traits(EventRejectCause::InvalidAuthority)
        && super::super::traits(EventRejectCause::InvalidStructure)
        && super::super::traits(EventRejectCause::BoundExceeded)
        && super::super::traits(EventRejectCause::InvalidActionability)
        && super::super::traits(EventRejectCause::Clock(ClockError::NotReady))
}

fn drop_variants() -> bool {
    super::super::traits(EventDropCause::RateLimited)
        && super::super::traits(EventDropCause::SynchronizationPressure)
        && super::super::traits(EventDropCause::AllocationFailure)
        && super::super::traits(EventDropCause::ChargePreparationFailure)
        && super::super::traits(EventDropCause::RenderingFailure)
}

fn closed_variants() -> bool {
    super::super::traits(EventClosedCause::StreamClosing)
        && super::super::traits(EventClosedCause::StreamClosed)
        && super::super::traits(EventClosedCause::StreamReplaced)
        && super::super::traits(EventClosedCause::ClockClosing)
        && super::super::traits(EventClosedCause::ClockClosed)
        && super::super::traits(EventClosedCause::ClockReplaced)
        && super::super::traits(EventClosedCause::SequenceExhausted)
}

fn admission_outcome_variants() -> bool {
    let first = EventSequence::first();
    let accepted = EventAdmissionOutcome::Accepted(first);
    let rejected =
        EventAdmissionOutcome::Rejected(EventRejectCause::StreamNotReady);
    let dropped = EventAdmissionOutcome::Dropped(EventDropCause::RateLimited);
    let closed = EventAdmissionOutcome::Closed(EventClosedCause::StreamClosing);

    super::super::traits(accepted)
        && super::super::traits(rejected)
        && super::super::traits(dropped)
        && super::super::traits(closed)
        && accepted.accepted_sequence() == Some(first)
        && rejected.accepted_sequence().is_none()
        && mapped_clock_outcomes()
}

const fn mapped_clock_outcomes() -> bool {
    closed_code(EventAdmissionOutcome::from_clock_error(ClockError::Closing))
        == 4
        && closed_code(EventAdmissionOutcome::from_clock_error(
            ClockError::Closed,
        )) == 5
        && closed_code(EventAdmissionOutcome::from_clock_error(
            ClockError::StaleGeneration,
        )) == 6
        && rejected_clock(EventAdmissionOutcome::from_clock_error(
            ClockError::NotReady,
        ))
}

const fn closed_code(outcome: EventAdmissionOutcome) -> u8 {
    match outcome {
        EventAdmissionOutcome::Closed(cause) => match cause {
            EventClosedCause::StreamClosing => 1,
            EventClosedCause::StreamClosed => 2,
            EventClosedCause::StreamReplaced => 3,
            EventClosedCause::ClockClosing => 4,
            EventClosedCause::ClockClosed => 5,
            EventClosedCause::ClockReplaced => 6,
            EventClosedCause::SequenceExhausted => 7,
        },
        EventAdmissionOutcome::Accepted(_)
        | EventAdmissionOutcome::Rejected(_)
        | EventAdmissionOutcome::Dropped(_) => 0,
    }
}

const fn rejected_clock(outcome: EventAdmissionOutcome) -> bool {
    match outcome {
        EventAdmissionOutcome::Rejected(EventRejectCause::Clock(
            ClockError::NotReady,
        )) => true,
        EventAdmissionOutcome::Accepted(_)
        | EventAdmissionOutcome::Rejected(_)
        | EventAdmissionOutcome::Dropped(_)
        | EventAdmissionOutcome::Closed(_) => false,
    }
}

fn error_variants() -> bool {
    let Some(bound) = extract_bound_error() else {
        return false;
    };
    let values = [
        EventError::Bound(bound),
        EventError::ChargeOwnerMismatch,
        EventError::DescriptorContractMismatch,
        EventError::UnsupportedContract,
        EventError::IncompatibleContract,
        EventError::ProposalOwnerMismatch,
        EventError::AdmissionScopeMismatch,
        EventError::InvalidProposalStructure,
        EventError::InvalidProposalClassification,
        EventError::UnassignedSequence,
        EventError::SequenceExhausted,
        EventError::InvalidCounterSaturation,
        EventError::InvalidRetainedRange,
        EventError::SnapshotBoundExceeded,
        EventError::InvalidSnapshotMetadata,
        EventError::InvalidStartupRecord,
        EventError::EmptyCanonicalBytes,
        EventError::NonAsciiCanonicalBytes,
        EventError::InvalidCanonicalFraming,
        EventError::CanonicalLengthOverflow,
        EventError::Clock(ClockError::NotReady),
    ];
    let traits_hold = values.into_iter().all(super::super::traits);
    let error = EventError::InvalidStartupRecord;
    traits_hold && super::super::error_traits(&error)
}

const fn extract_bound_error() -> Option<BoundError> {
    match StartupEventLimits::new(
        super::STREAM_INSTANCE,
        9,
        8,
        32,
        2_048,
        4,
        2_048,
    ) {
        Err(EventError::Bound(error)) => Some(error),
        Err(_) | Ok(_) => None,
    }
}

pub const fn event_error_code(error: EventError) -> u8 {
    match error {
        EventError::Bound(_) => 1,
        EventError::ChargeOwnerMismatch => 2,
        EventError::DescriptorContractMismatch => 3,
        EventError::UnsupportedContract => 4,
        EventError::IncompatibleContract => 5,
        EventError::ProposalOwnerMismatch => 6,
        EventError::AdmissionScopeMismatch => 7,
        EventError::InvalidProposalStructure => 21,
        EventError::InvalidProposalClassification => 8,
        EventError::UnassignedSequence => 9,
        EventError::SequenceExhausted => 10,
        EventError::InvalidCounterSaturation => 11,
        EventError::InvalidRetainedRange => 12,
        EventError::SnapshotBoundExceeded => 13,
        EventError::InvalidSnapshotMetadata => 14,
        EventError::InvalidStartupRecord => 15,
        EventError::EmptyCanonicalBytes => 16,
        EventError::NonAsciiCanonicalBytes => 17,
        EventError::InvalidCanonicalFraming => 18,
        EventError::CanonicalLengthOverflow => 19,
        EventError::Clock(_) => 20,
    }
}
