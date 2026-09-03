use crate::lifecycle::{LifecycleIdentity, ReadinessOutcome};
use reovim_foundation::bound::BoundError;

use super::counts::admit_counts;
use super::remediation::{classification_matches, classify};
use super::{
    Actionability, BootstrapHealth, EventError, EventRejectCause, Severity,
    StartupComponentCounts, StartupEventDescriptor, StartupProposal,
};

#[derive(Clone, Copy)]
enum ProposalValidationFailure {
    Authority,
    Bound(BoundError),
    Structure,
    Actionability,
}

impl crate::event::StartupProposal {
    /// Constructs and validates one unadmitted startup proposal.
    ///
    /// Severity and Actionability are derived only from `readiness`. This
    /// constructor assigns no producer, correlation, sequence, timestamp, or
    /// canonical bytes.
    ///
    /// # Errors
    ///
    /// Returns the exact pre-admission rejection cause before work.
    pub const fn new(
        descriptor: &StartupEventDescriptor,
        lifecycle_owner: LifecycleIdentity,
        readiness: ReadinessOutcome,
        counts: StartupComponentCounts,
        bootstrap_health: BootstrapHealth,
    ) -> Result<Self, EventRejectCause> {
        match validate_base(descriptor, lifecycle_owner, readiness, counts) {
            Ok(()) => {}
            Err(error) => return Err(error.reject_cause()),
        }
        let (severity, actionability) = classify(descriptor, readiness);
        Ok(Self {
            lifecycle_owner,
            readiness,
            counts,
            bootstrap_health,
            severity,
            actionability,
        })
    }

    /// Revalidates this proposal against one exact admission descriptor.
    ///
    /// # Errors
    ///
    /// Returns the exact pre-admission rejection cause.
    pub const fn validate_for(
        self,
        descriptor: &StartupEventDescriptor,
    ) -> Result<(), EventRejectCause> {
        match validate_complete(self, descriptor) {
            Ok(()) => Ok(()),
            Err(error) => Err(error.reject_cause()),
        }
    }

    /// Returns the lifecycle owner stamp carried by this proposal.
    #[must_use]
    pub const fn lifecycle_owner(self) -> LifecycleIdentity {
        self.lifecycle_owner
    }

    /// Returns the startup readiness outcome.
    #[must_use]
    pub const fn readiness(self) -> ReadinessOutcome {
        self.readiness
    }

    /// Returns the checked component counts.
    #[must_use]
    pub const fn counts(self) -> StartupComponentCounts {
        self.counts
    }

    /// Returns the fixed bootstrap-health summary marker.
    #[must_use]
    pub const fn bootstrap_health(self) -> BootstrapHealth {
        self.bootstrap_health
    }

    /// Returns severity derived from readiness.
    #[must_use]
    pub const fn severity(self) -> Severity {
        self.severity
    }

    /// Returns Actionability derived from readiness and the fixed catalog.
    #[must_use]
    pub const fn actionability(self) -> Actionability {
        self.actionability
    }

    pub(super) const fn validate_for_record(
        self,
        descriptor: &StartupEventDescriptor,
    ) -> Result<(), EventError> {
        match validate_complete(self, descriptor) {
            Ok(()) => Ok(()),
            Err(error) => Err(error.event_error()),
        }
    }
}

impl ProposalValidationFailure {
    const fn event_error(self) -> EventError {
        match self {
            Self::Authority => EventError::ProposalOwnerMismatch,
            Self::Bound(error) => EventError::Bound(error),
            Self::Structure => EventError::InvalidProposalStructure,
            Self::Actionability => EventError::InvalidProposalClassification,
        }
    }

    const fn reject_cause(self) -> EventRejectCause {
        match self {
            Self::Authority => EventRejectCause::InvalidAuthority,
            Self::Bound(_) => EventRejectCause::BoundExceeded,
            Self::Structure => EventRejectCause::InvalidStructure,
            Self::Actionability => EventRejectCause::InvalidActionability,
        }
    }
}

const fn validate_complete(
    proposal: StartupProposal,
    descriptor: &StartupEventDescriptor,
) -> Result<(), ProposalValidationFailure> {
    match validate_base(
        descriptor,
        proposal.lifecycle_owner,
        proposal.readiness,
        proposal.counts,
    ) {
        Ok(()) => {}
        Err(error) => return Err(error),
    }
    if !classification_matches(
        descriptor,
        proposal.readiness,
        proposal.severity,
        proposal.actionability,
    ) {
        return Err(ProposalValidationFailure::Actionability);
    }
    Ok(())
}

const fn validate_base(
    descriptor: &StartupEventDescriptor,
    lifecycle_owner: LifecycleIdentity,
    readiness: ReadinessOutcome,
    counts: StartupComponentCounts,
) -> Result<(), ProposalValidationFailure> {
    let expected = descriptor.identities().lifecycle_owner();
    if !expected.same_generation(lifecycle_owner) {
        return Err(ProposalValidationFailure::Authority);
    }
    match admit_counts(descriptor.limits().component_counts(), counts) {
        Ok(()) => {}
        Err(error) => return Err(ProposalValidationFailure::Bound(error)),
    }
    if !counts_match_readiness(readiness, counts) {
        return Err(ProposalValidationFailure::Structure);
    }
    Ok(())
}

const fn counts_match_readiness(
    readiness: ReadinessOutcome,
    counts: StartupComponentCounts,
) -> bool {
    match readiness {
        ReadinessOutcome::Ready => {
            counts.degraded().value() == 0 && counts.unavailable().value() == 0
        }
        ReadinessOutcome::DiagnosticsDegraded => {
            counts.degraded().value() > 0 || counts.unavailable().value() > 0
        }
        ReadinessOutcome::ActivationFailed(_) => {
            counts.unavailable().value() > 0
        }
    }
}
