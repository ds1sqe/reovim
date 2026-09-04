//! Borrowed Representation lowering and returned-plan validation boundary.

use core::fmt;

use crate::buffer::plan::{BufferPlanDescriptor, BufferTransactionPlan};
use crate::cancellation::{
    CancellationState, DeadlineState, InstallationOutcome,
};

use super::bounds::SemanticPolicyCallCharge;
use super::descriptor::{RepresentationPolicyDescriptor, SemanticEditability};
use super::domain::DomainValidationReceipt;
use super::intent::SemanticIntent;
use super::policy::{
    SemanticPolicyContractError, SemanticPolicyFailure,
    SemanticPolicyRequestError,
};

/// Provider disposition carried by a Representation lowering candidate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepresentationLoweringCandidateStatus {
    /// Policy produced one complete Buffer Transaction plan candidate.
    Lowered,
    /// Policy produced an expected semantic non-result.
    Rejected,
}

/// Move-only candidate returned by concrete Representation policy.
pub struct RepresentationLoweringCandidate {
    plan: Option<BufferTransactionPlan>,
    failure: Option<SemanticPolicyFailure>,
}

impl RepresentationLoweringCandidate {
    /// Constructs an expected semantic non-result with no plan.
    #[must_use]
    pub const fn rejected(failure: SemanticPolicyFailure) -> Self {
        Self { plan: None, failure: Some(failure) }
    }

    /// Returns the provider's candidate disposition.
    #[must_use]
    pub const fn status(&self) -> RepresentationLoweringCandidateStatus {
        if self.plan.is_some() {
            RepresentationLoweringCandidateStatus::Lowered
        } else {
            RepresentationLoweringCandidateStatus::Rejected
        }
    }

    /// Borrows the complete plan candidate, when lowering succeeded.
    #[must_use]
    pub const fn plan(&self) -> Option<&BufferTransactionPlan> {
        self.plan.as_ref()
    }

    /// Returns the expected semantic failure, when lowering rejected.
    #[must_use]
    pub const fn failure(&self) -> Option<SemanticPolicyFailure> {
        self.failure
    }

    /// Validates this provider candidate against the retained request.
    #[must_use]
    pub fn finish(
        self,
        request: RepresentationLoweringRequest<'_>,
        provider: &RepresentationPolicyDescriptor,
        installation: InstallationOutcome,
    ) -> RepresentationLoweringOutcome {
        if request.representation != provider {
            return self.into_contract_violation(
                SemanticPolicyContractError::ProviderDescriptorMismatch,
            );
        }
        match (self.plan, self.failure) {
            (Some(plan), None) => {
                let violation = validate_returned_plan(request, &plan).err();
                let failure = installation_failure(violation, installation);
                RepresentationLoweringOutcome {
                    plan: Some(plan),
                    failure,
                    violation,
                }
            }
            (None, Some(failure)) => RepresentationLoweringOutcome {
                plan: None,
                failure: Some(failure),
                violation: None,
            },
            (plan, _) => RepresentationLoweringOutcome {
                plan,
                failure: None,
                violation: Some(
                    SemanticPolicyContractError::LoweringCandidateShape,
                ),
            },
        }
    }

    fn into_contract_violation(
        self,
        violation: SemanticPolicyContractError,
    ) -> RepresentationLoweringOutcome {
        RepresentationLoweringOutcome {
            plan: self.plan,
            failure: None,
            violation: Some(violation),
        }
    }
}

impl fmt::Debug for RepresentationLoweringCandidate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RepresentationLoweringCandidate")
            .field("status", &self.status())
            .field("plan", &self.plan)
            .field("failure", &self.failure)
            .finish()
    }
}

impl From<BufferTransactionPlan> for RepresentationLoweringCandidate {
    fn from(plan: BufferTransactionPlan) -> Self {
        Self { plan: Some(plan), failure: None }
    }
}

/// Runtime disposition of a Representation lowering candidate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepresentationLoweringStatus {
    /// The complete plan passed every retained-request validation.
    Lowered,
    /// Representation policy returned an expected semantic non-result.
    Rejected,
    /// Provider output did not match the retained request or descriptor.
    ContractViolation,
}

/// Move-only Runtime classification retaining any returned plan candidate.
pub struct RepresentationLoweringOutcome {
    plan: Option<BufferTransactionPlan>,
    failure: Option<SemanticPolicyFailure>,
    violation: Option<SemanticPolicyContractError>,
}

impl RepresentationLoweringOutcome {
    /// Returns the Runtime's validated disposition.
    #[must_use]
    pub const fn status(&self) -> RepresentationLoweringStatus {
        if self.violation.is_some() {
            RepresentationLoweringStatus::ContractViolation
        } else if self.failure.is_some() {
            RepresentationLoweringStatus::Rejected
        } else {
            RepresentationLoweringStatus::Lowered
        }
    }

    /// Borrows the provider's complete plan, including on violation.
    #[must_use]
    pub const fn plan(&self) -> Option<&BufferTransactionPlan> {
        self.plan.as_ref()
    }

    /// Returns the expected semantic failure, when rejected.
    #[must_use]
    pub const fn failure(&self) -> Option<SemanticPolicyFailure> {
        self.failure
    }

    /// Returns the provider contract violation, when validation failed.
    #[must_use]
    pub const fn violation(&self) -> Option<SemanticPolicyContractError> {
        self.violation
    }

    /// Returns ownership of the provider's complete plan, when supplied.
    #[must_use]
    pub fn into_plan(self) -> Option<BufferTransactionPlan> {
        self.plan
    }
}

impl fmt::Debug for RepresentationLoweringOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RepresentationLoweringOutcome")
            .field("status", &self.status())
            .field("plan", &self.plan)
            .field("failure", &self.failure)
            .field("violation", &self.violation)
            .finish()
    }
}

/// Immutable invocation-scoped Representation lowering request.
#[derive(Clone, Copy)]
pub struct RepresentationLoweringRequest<'a> {
    representation: &'a RepresentationPolicyDescriptor,
    intent: &'a SemanticIntent,
    validation: &'a DomainValidationReceipt,
    plan: &'a BufferPlanDescriptor,
    charge: SemanticPolicyCallCharge,
    cancellation: CancellationState,
    deadline: DeadlineState,
}

impl<'a> RepresentationLoweringRequest<'a> {
    /// Binds retained policy inputs to exact lowering call terms.
    ///
    /// # Errors
    ///
    /// Returns contract, read-only, bound, cancellation, or deadline failure
    /// before provider invocation.
    pub const fn new(
        representation: &'a RepresentationPolicyDescriptor,
        intent: &'a SemanticIntent,
        validation: &'a DomainValidationReceipt,
        plan: &'a BufferPlanDescriptor,
        charge: SemanticPolicyCallCharge,
        cancellation: CancellationState,
        deadline: DeadlineState,
    ) -> Result<Self, SemanticPolicyRequestError> {
        match validate_binding(representation, intent, validation, plan) {
            Ok(()) => {}
            Err(error) => {
                return Err(SemanticPolicyRequestError::Contract(error));
            }
        }
        if !is_transactional(representation.editability()) {
            return Err(SemanticPolicyRequestError::ReadOnly);
        }
        match representation.lowering().bounds().validate_charge(charge) {
            Ok(()) => {}
            Err(error) => return Err(SemanticPolicyRequestError::Bound(error)),
        }
        match cancellation {
            CancellationState::Open => {}
            CancellationState::Requested(cause) => {
                return Err(SemanticPolicyRequestError::Cancelled(cause));
            }
        }
        match deadline {
            DeadlineState::Pending => {}
            DeadlineState::Reached => {
                return Err(SemanticPolicyRequestError::DeadlineReached);
            }
        }
        Ok(Self {
            representation,
            intent,
            validation,
            plan,
            charge,
            cancellation,
            deadline,
        })
    }

    /// Returns the selected Representation descriptor.
    #[must_use]
    pub const fn representation(self) -> &'a RepresentationPolicyDescriptor {
        self.representation
    }

    /// Returns the immutable retained semantic Intent.
    #[must_use]
    pub const fn intent(self) -> &'a SemanticIntent {
        self.intent
    }

    /// Returns the Runtime-minted Domain validation receipt.
    #[must_use]
    pub const fn validation(self) -> &'a DomainValidationReceipt {
        self.validation
    }

    /// Returns the exact descriptor governing the complete output plan.
    #[must_use]
    pub const fn plan(self) -> &'a BufferPlanDescriptor {
        self.plan
    }

    /// Returns the validated policy-call reservation.
    #[must_use]
    pub const fn charge(self) -> SemanticPolicyCallCharge {
        self.charge
    }

    /// Returns cancellation state observed before callback entry.
    #[must_use]
    pub const fn cancellation(self) -> CancellationState {
        self.cancellation
    }

    /// Returns deadline state observed before callback entry.
    #[must_use]
    pub const fn deadline(self) -> DeadlineState {
        self.deadline
    }
}

impl fmt::Debug for RepresentationLoweringRequest<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RepresentationLoweringRequest")
            .field("representation", &self.representation.stamp())
            .field("intent", &self.intent)
            .field("validation", &self.validation)
            .field("plan", &self.plan.contract())
            .field("charge", &self.charge)
            .field("cancellation", &self.cancellation)
            .field("deadline", &self.deadline)
            .finish()
    }
}

/// Public complete-lowering callback implemented by selected policy.
///
/// Assembly selects this object only together with `Send + Sync`; keeping the
/// callback trait separate also permits direct deterministic test providers.
pub trait RepresentationPolicyPort {
    /// Returns the immutable descriptor governing this provider instance.
    fn descriptor(&self) -> &RepresentationPolicyDescriptor;

    /// Returns one complete plan candidate or expected semantic non-result.
    fn lower(
        &self,
        request: RepresentationLoweringRequest<'_>,
    ) -> RepresentationLoweringCandidate;
}

const fn validate_binding(
    representation: &RepresentationPolicyDescriptor,
    intent: &SemanticIntent,
    validation: &DomainValidationReceipt,
    plan: &BufferPlanDescriptor,
) -> Result<(), SemanticPolicyContractError> {
    if !validation.matches_intent(representation.stamp().domain(), intent) {
        return Err(SemanticPolicyContractError::ValidationReceiptMismatch);
    }
    if !representation
        .stamp()
        .same_generation(intent.stamp().projection().representation())
    {
        return Err(SemanticPolicyContractError::IntentRepresentationMismatch);
    }
    if !representation
        .coordinate()
        .stamp()
        .same_incarnation(intent.stamp().coordinate())
    {
        return Err(SemanticPolicyContractError::IntentCoordinateMismatch);
    }
    if !representation.plan().matches_descriptor(plan) {
        return Err(SemanticPolicyContractError::PlanDescriptorMismatch);
    }
    if !plan.owner().same_generation(intent.stamp().plan().owner()) {
        return Err(SemanticPolicyContractError::PlanOwnerMismatch);
    }
    Ok(())
}

const fn validate_returned_plan(
    request: RepresentationLoweringRequest<'_>,
    plan: &BufferTransactionPlan,
) -> Result<(), SemanticPolicyContractError> {
    if !plan.stamp().same_plan(*request.intent.stamp().plan()) {
        return Err(SemanticPolicyContractError::ReturnedPlanStampMismatch);
    }
    if !request.intent.stamp().target().same_state(plan.target()) {
        return Err(SemanticPolicyContractError::ReturnedPlanTargetMismatch);
    }
    let reserved = request.charge.result_bytes().value();
    let actual = plan.owned_bytes().value();
    if actual > reserved {
        return Err(SemanticPolicyContractError::ReturnedPlanChargeExceeded {
            reserved,
            actual,
        });
    }
    match request.plan.validate_plan(plan) {
        Ok(()) => Ok(()),
        Err(error) => {
            Err(SemanticPolicyContractError::ReturnedPlanInvalid(error))
        }
    }
}

const fn installation_failure(
    violation: Option<SemanticPolicyContractError>,
    installation: InstallationOutcome,
) -> Option<SemanticPolicyFailure> {
    match (violation, installation) {
        (None, InstallationOutcome::Fenced(cause)) => {
            Some(SemanticPolicyFailure::from_fence(cause))
        }
        _ => None,
    }
}

const fn is_transactional(editability: SemanticEditability) -> bool {
    match editability {
        SemanticEditability::Transactional => true,
        SemanticEditability::ReadOnly => false,
    }
}
