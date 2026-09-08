//! Borrowed Domain validation request and provider candidate boundary.

use core::fmt;

use crate::buffer::BufferStateStamp;
use crate::buffer::plan::BufferPlanStamp;
use crate::cancellation::{
    CancellationState, DeadlineState, InstallationOutcome,
};
use crate::resource::state::{ResourceStateModelBinding, ResourceWorkingState};

use super::bounds::{SemanticIntentShape, SemanticPolicyCallCharge};
use super::descriptor::{
    DomainPolicyDescriptor, RepresentationPolicyDescriptor,
};
use super::identity::{SemanticIntentIdentity, SemanticRequestIdentity};
use super::intent::{SemanticIntent, SemanticIntentCarrierStamp};
use super::policy::{
    SemanticPolicyContractError, SemanticPolicyFailure,
    SemanticPolicyRequestError,
};
use super::stamp::{
    CoordinateSpaceStamp, DomainPolicyStamp, ProjectionStamp,
    SemanticCallerStamp, SemanticIntentStamp,
};
use super::state::SemanticPolicyInput;

/// Exact Domain validation claim over one retained semantic Intent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SemanticIntentValidationStamp {
    domain: DomainPolicyStamp,
    intent: SemanticIntentIdentity,
    request: SemanticRequestIdentity,
    carrier: SemanticIntentCarrierStamp,
    shape: SemanticIntentShape,
    state_model: ResourceStateModelBinding,
    target: BufferStateStamp,
    caller: SemanticCallerStamp,
    plan: BufferPlanStamp,
    projection: ProjectionStamp,
    coordinate: CoordinateSpaceStamp,
}

impl SemanticIntentValidationStamp {
    /// Constructs one provider-supplied validation claim.
    #[must_use]
    pub const fn new(
        domain: DomainPolicyStamp,
        semantic: &SemanticIntentStamp,
        carrier: SemanticIntentCarrierStamp,
        shape: SemanticIntentShape,
        state_model: &ResourceStateModelBinding,
    ) -> Self {
        Self {
            domain,
            intent: semantic.intent(),
            request: semantic.request(),
            carrier,
            shape,
            state_model: *state_model,
            target: *semantic.target(),
            caller: semantic.caller(),
            plan: *semantic.plan(),
            projection: *semantic.projection(),
            coordinate: semantic.coordinate(),
        }
    }

    /// Returns the claimed Domain policy generation.
    #[must_use]
    pub const fn domain(self) -> DomainPolicyStamp {
        self.domain
    }

    /// Returns the unique claimed semantic Intent identity.
    #[must_use]
    pub const fn intent(self) -> SemanticIntentIdentity {
        self.intent
    }

    /// Returns the claimed semantic request identity.
    #[must_use]
    pub const fn request(self) -> SemanticRequestIdentity {
        self.request
    }

    /// Returns the claimed Intent carrier type and schema.
    #[must_use]
    pub const fn carrier(self) -> SemanticIntentCarrierStamp {
        self.carrier
    }

    /// Returns the claimed Intent structural and byte shape.
    #[must_use]
    pub const fn shape(self) -> SemanticIntentShape {
        self.shape
    }

    /// Reports whether this claim names one exact retained Intent.
    #[must_use]
    pub fn matches_intent(
        self,
        domain: &DomainPolicyDescriptor,
        intent: &SemanticIntent,
    ) -> bool {
        self.matches_selection(domain.stamp(), intent, domain.state_model())
    }

    /// Returns the native model against which meaning was validated.
    #[must_use]
    pub const fn state_model(&self) -> &ResourceStateModelBinding {
        &self.state_model
    }

    /// Returns the exact observed Buffer/History state that was validated.
    #[must_use]
    pub const fn target(&self) -> &BufferStateStamp {
        &self.target
    }

    pub(super) fn matches_selection(
        self,
        domain: DomainPolicyStamp,
        intent: &SemanticIntent,
        state_model: &ResourceStateModelBinding,
    ) -> bool {
        self.domain.same_generation(domain)
            && self.intent_stamp().same_request(intent.stamp())
            && same_carrier(self.carrier, intent.carrier())
            && same_shape(self.shape, intent.shape())
            && self.state_model == *state_model
    }

    const fn intent_stamp(&self) -> SemanticIntentStamp {
        SemanticIntentStamp::new(
            self.intent,
            self.caller,
            self.request,
            self.target,
            self.plan,
            self.projection,
            self.coordinate,
        )
    }
}

/// Candidate returned by concrete Domain validation policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DomainValidationCandidate {
    validation: Option<SemanticIntentValidationStamp>,
    failure: Option<SemanticPolicyFailure>,
}

/// Provider disposition carried by a Domain validation candidate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DomainValidationCandidateStatus {
    /// Policy claims semantic validity for the exact retained Intent.
    Validated,
    /// Policy rejected semantic meaning without Buffer state change.
    Rejected,
}

impl DomainValidationCandidate {
    /// Constructs a validation claim for one exact request stamp.
    #[must_use]
    pub const fn validated(stamp: SemanticIntentValidationStamp) -> Self {
        Self { validation: Some(stamp), failure: None }
    }

    /// Constructs an expected semantic non-result.
    #[must_use]
    pub const fn rejected(failure: SemanticPolicyFailure) -> Self {
        Self { validation: None, failure: Some(failure) }
    }

    /// Returns the provider's candidate disposition.
    #[must_use]
    pub const fn status(self) -> DomainValidationCandidateStatus {
        if self.validation.is_some() {
            DomainValidationCandidateStatus::Validated
        } else {
            DomainValidationCandidateStatus::Rejected
        }
    }

    /// Returns the claimed validation stamp, when validated.
    #[must_use]
    pub const fn validation(self) -> Option<SemanticIntentValidationStamp> {
        self.validation
    }

    /// Returns the expected semantic failure, when rejected.
    #[must_use]
    pub const fn failure(self) -> Option<SemanticPolicyFailure> {
        self.failure
    }
}

/// Runtime-validated proof required before Representation lowering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DomainValidationReceipt {
    stamp: SemanticIntentValidationStamp,
}

impl DomainValidationReceipt {
    const fn new(stamp: &SemanticIntentValidationStamp) -> Self {
        Self { stamp: *stamp }
    }

    /// Returns the exact Runtime-validated Domain claim.
    #[must_use]
    pub const fn stamp(self) -> SemanticIntentValidationStamp {
        self.stamp
    }

    /// Reports whether this receipt belongs to one exact retained Intent.
    #[must_use]
    pub fn matches_intent(
        self,
        domain: &DomainPolicyDescriptor,
        intent: &SemanticIntent,
    ) -> bool {
        self.stamp.matches_intent(domain, intent)
    }
}

/// Runtime classification of one Domain validation candidate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DomainValidationOutcome {
    receipt: Option<DomainValidationReceipt>,
    failure: Option<SemanticPolicyFailure>,
    violation: Option<SemanticPolicyContractError>,
}

/// Runtime disposition of a Domain validation candidate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DomainValidationStatus {
    /// Exact provider output was validated and a receipt was minted.
    Validated,
    /// Domain policy returned an expected semantic non-result.
    Rejected,
    /// Provider output did not match the retained request or descriptor.
    ContractViolation,
}

impl DomainValidationOutcome {
    const fn validated(receipt: &DomainValidationReceipt) -> Self {
        Self { receipt: Some(*receipt), failure: None, violation: None }
    }

    const fn rejected(failure: SemanticPolicyFailure) -> Self {
        Self { receipt: None, failure: Some(failure), violation: None }
    }

    const fn contract_violation(
        violation: SemanticPolicyContractError,
    ) -> Self {
        Self { receipt: None, failure: None, violation: Some(violation) }
    }

    /// Returns the Runtime's validated disposition.
    #[must_use]
    pub const fn status(self) -> DomainValidationStatus {
        if self.receipt.is_some() {
            DomainValidationStatus::Validated
        } else if self.failure.is_some() {
            DomainValidationStatus::Rejected
        } else {
            DomainValidationStatus::ContractViolation
        }
    }

    /// Returns the Runtime-minted receipt, when validated.
    #[must_use]
    pub const fn receipt(self) -> Option<DomainValidationReceipt> {
        self.receipt
    }

    /// Returns the expected semantic failure, when rejected.
    #[must_use]
    pub const fn failure(self) -> Option<SemanticPolicyFailure> {
        self.failure
    }

    /// Returns the provider contract violation, when rejected structurally.
    #[must_use]
    pub const fn violation(self) -> Option<SemanticPolicyContractError> {
        self.violation
    }
}

/// Immutable invocation-scoped Domain validation request.
pub struct DomainValidationRequest<'a, State> {
    domain: &'a DomainPolicyDescriptor,
    input: SemanticPolicyInput<'a, State>,
    charge: SemanticPolicyCallCharge,
    cancellation: CancellationState,
    deadline: DeadlineState,
}

impl<State> Copy for DomainValidationRequest<'_, State> {}

impl<State> Clone for DomainValidationRequest<'_, State> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, State> DomainValidationRequest<'a, State> {
    /// Binds a retained Intent to exact Domain validation call terms.
    ///
    /// # Errors
    ///
    /// Returns contract, bound, cancellation, or deadline failure before
    /// provider invocation.
    pub fn new(
        domain: &'a DomainPolicyDescriptor,
        representation: &'a RepresentationPolicyDescriptor,
        input: SemanticPolicyInput<'a, State>,
        charge: SemanticPolicyCallCharge,
        cancellation: CancellationState,
        deadline: DeadlineState,
    ) -> Result<Self, SemanticPolicyRequestError> {
        input
            .validate(domain.state_model())
            .map_err(SemanticPolicyRequestError::Contract)?;
        if domain.state_model() != representation.state_model() {
            return Err(SemanticPolicyRequestError::Contract(
                SemanticPolicyContractError::WorkingStateModelMismatch,
            ));
        }
        match validate_binding(domain, representation, input.intent) {
            Ok(()) => {}
            Err(error) => {
                return Err(SemanticPolicyRequestError::Contract(error));
            }
        }
        match domain.validation().bounds().validate_charge(charge) {
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
        Ok(Self { domain, input, charge, cancellation, deadline })
    }

    /// Returns the selected Domain descriptor.
    #[must_use]
    pub const fn domain(self) -> &'a DomainPolicyDescriptor {
        self.domain
    }

    /// Returns the immutable retained semantic Intent.
    #[must_use]
    pub const fn intent(self) -> &'a SemanticIntent {
        self.input.intent
    }

    /// Returns actual immutable state used for semantic meaning validation.
    #[must_use]
    pub const fn state(self) -> &'a ResourceWorkingState<'a, State> {
        self.input.state
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

    /// Returns the exact validation stamp a conforming provider may claim.
    #[must_use]
    pub const fn validation_stamp(self) -> SemanticIntentValidationStamp {
        SemanticIntentValidationStamp::new(
            self.domain.stamp(),
            self.input.intent.stamp(),
            self.input.intent.carrier(),
            self.input.intent.shape(),
            &self.input.state.model,
        )
    }

    /// Constructs an exact validated candidate for this retained request.
    #[must_use]
    pub const fn validated_candidate(self) -> DomainValidationCandidate {
        DomainValidationCandidate::validated(self.validation_stamp())
    }

    /// Invokes only the selected provider, outside Buffer commit ownership.
    ///
    /// Call `finish` afterward with the current installation fence; invocation
    /// alone neither mints a validation receipt nor accepts an edit.
    ///
    /// # Errors
    ///
    /// Rejects another provider descriptor before it may inspect native state.
    pub fn invoke<Provider>(
        self,
        provider: &Provider,
    ) -> Result<DomainValidationCandidate, SemanticPolicyContractError>
    where
        Provider: DomainPolicyPort<State = State> + ?Sized,
    {
        if self.domain != provider.descriptor() {
            return Err(
                SemanticPolicyContractError::ProviderDescriptorMismatch,
            );
        }
        Ok(provider.validate(self))
    }

    /// Validates provider identity and candidate before minting a receipt.
    #[must_use]
    pub fn finish(
        self,
        provider: &DomainPolicyDescriptor,
        candidate: DomainValidationCandidate,
        installation: InstallationOutcome,
    ) -> DomainValidationOutcome {
        if self.domain != provider {
            return DomainValidationOutcome::contract_violation(
                SemanticPolicyContractError::ProviderDescriptorMismatch,
            );
        }
        if let Some(stamp) = candidate.validation() {
            return self.finish_validation(&stamp, installation);
        }
        let Some(failure) = candidate.failure() else {
            return DomainValidationOutcome::contract_violation(
                SemanticPolicyContractError::ValidationStampMismatch,
            );
        };
        DomainValidationOutcome::rejected(failure)
    }

    fn finish_validation(
        self,
        stamp: &SemanticIntentValidationStamp,
        installation: InstallationOutcome,
    ) -> DomainValidationOutcome {
        if !stamp.matches_intent(self.domain, self.input.intent) {
            return DomainValidationOutcome::contract_violation(
                SemanticPolicyContractError::ValidationStampMismatch,
            );
        }
        match installation {
            InstallationOutcome::Current => DomainValidationOutcome::validated(
                &DomainValidationReceipt::new(stamp),
            ),
            InstallationOutcome::Fenced(cause) => {
                DomainValidationOutcome::rejected(
                    SemanticPolicyFailure::from_fence(cause),
                )
            }
        }
    }
}

impl<State> fmt::Debug for DomainValidationRequest<'_, State> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DomainValidationRequest")
            .field("domain", &self.domain.stamp())
            .field("intent", &self.input.intent)
            .field("state", &self.input.state)
            .field("charge", &self.charge)
            .field("cancellation", &self.cancellation)
            .field("deadline", &self.deadline)
            .finish()
    }
}

/// Public Domain validation callback implemented by selected policy.
///
/// Assembly selects this object only together with `Send + Sync`; keeping the
/// callback trait separate also permits direct deterministic test providers.
pub trait DomainPolicyPort {
    /// Public immutable native state/read surface consumed by this policy.
    type State;

    /// Returns the immutable descriptor governing this provider instance.
    fn descriptor(&self) -> &DomainPolicyDescriptor;

    /// Returns a candidate from one immutable invocation-scoped request.
    fn validate(
        &self,
        request: DomainValidationRequest<'_, Self::State>,
    ) -> DomainValidationCandidate;
}

const fn validate_binding(
    domain: &DomainPolicyDescriptor,
    representation: &RepresentationPolicyDescriptor,
    intent: &SemanticIntent,
) -> Result<(), SemanticPolicyContractError> {
    let projection = intent.stamp().projection();
    if !domain.stamp().same_generation(projection.representation().domain()) {
        return Err(SemanticPolicyContractError::IntentDomainMismatch);
    }
    if !representation.stamp().same_generation(projection.representation()) {
        return Err(SemanticPolicyContractError::IntentRepresentationMismatch);
    }
    if !representation
        .coordinate()
        .stamp()
        .same_incarnation(intent.stamp().coordinate())
    {
        return Err(SemanticPolicyContractError::IntentCoordinateMismatch);
    }
    match super::intent::validate_retained(representation, intent) {
        Ok(()) => {}
        Err(error) => {
            return Err(SemanticPolicyContractError::RetainedIntentInvalid(
                error,
            ));
        }
    }
    Ok(())
}

const fn same_carrier(
    left: SemanticIntentCarrierStamp,
    right: SemanticIntentCarrierStamp,
) -> bool {
    left.type_identity() == right.type_identity()
        && left.schema_version() == right.schema_version()
}

const fn same_shape(
    left: SemanticIntentShape,
    right: SemanticIntentShape,
) -> bool {
    left.items().value() == right.items().value()
        && left.depth().value() == right.depth().value()
        && left.bytes().value() == right.bytes().value()
}
