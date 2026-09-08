use reovim_kernel_interface::cancellation::{
    CancellationCause, CancellationState, DeadlineState, FenceCause,
    InstallationOutcome,
};
use reovim_kernel_interface::semantic::domain::{
    DomainPolicyPort, DomainValidationCandidate,
    DomainValidationCandidateStatus, DomainValidationRequest,
    DomainValidationStatus, SemanticIntentValidationStamp,
};
use reovim_kernel_interface::semantic::identity::SemanticIntentIdentity;
use reovim_kernel_interface::semantic::policy::{
    SemanticPolicyContractError, SemanticPolicyFailure,
    SemanticPolicyRequestError,
};

use super::support::{self, SemanticFixture};

#[derive(Clone, Copy)]
enum Script {
    Inspect,
    WrongIntent,
    Reject(SemanticPolicyFailure),
}

struct ScriptedDomain<'a> {
    descriptor: &'a reovim_kernel_interface::semantic::descriptor::
        DomainPolicyDescriptor,
    script: Script,
}

impl ::reovim_kernel_interface::semantic::domain::DomainPolicyPort
    for ScriptedDomain<'_>
{
    type State = ();
    fn descriptor(
        &self,
    ) -> &reovim_kernel_interface::semantic::descriptor::DomainPolicyDescriptor
    {
        self.descriptor
    }

    fn validate(
        &self,
        request: DomainValidationRequest<'_, ()>,
    ) -> DomainValidationCandidate {
        match self.script {
            Script::Inspect
                if request.intent().bytes() == support::INTENT_BYTES =>
            {
                request.validated_candidate()
            }
            Script::Inspect => DomainValidationCandidate::rejected(
                SemanticPolicyFailure::InvalidIntent,
            ),
            Script::WrongIntent => wrong_intent_candidate(request),
            Script::Reject(failure) => {
                DomainValidationCandidate::rejected(failure)
            }
        }
    }
}

pub const fn const_cases() -> bool {
    crate::derive_witness::<
        ::reovim_kernel_interface::semantic::domain::
            SemanticIntentValidationStamp,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::semantic::domain::
            DomainValidationCandidate,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::semantic::domain::
            DomainValidationCandidateStatus,
    >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::semantic::domain::
                DomainValidationReceipt,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::semantic::domain::
                DomainValidationOutcome,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::semantic::domain::
                DomainValidationStatus,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::semantic::policy::
                SemanticPolicyFailure,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::semantic::policy::
                SemanticPolicyContractError,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::semantic::policy::
                SemanticPolicyRequestError,
        >()
        && crate::clone_copy_witness::<
            ::reovim_kernel_interface::semantic::domain::
                DomainValidationRequest<'static, ()>,
        >()
}

pub fn exercise() -> bool {
    valid_receipt()
        && semantic_rejection()
        && wrong_candidate()
        && wrong_provider()
        && post_callback_fence()
        && request_failures()
        && failure_vocabulary()
}

pub(super) fn validated_receipt(
    fixture: &SemanticFixture,
    intent: &reovim_kernel_interface::semantic::intent::SemanticIntent,
) -> Option<reovim_kernel_interface::semantic::domain::DomainValidationReceipt>
{
    let request = request(fixture, intent)?;
    let provider =
        ScriptedDomain { descriptor: &fixture.domain, script: Script::Inspect };
    request
        .finish(
            provider.descriptor(),
            invoke(&provider, request),
            InstallationOutcome::Current,
        )
        .receipt()
}

fn valid_receipt() -> bool {
    let Some(fixture) = support::fixture() else { return false };
    let Some(intent) = support::make_intent(&fixture, support::INTENT_BYTES)
    else {
        return false;
    };
    let Some(request) = request(&fixture, &intent) else { return false };
    let provider =
        ScriptedDomain { descriptor: &fixture.domain, script: Script::Inspect };
    let candidate = invoke(&provider, request);
    let candidate_fields = candidate.status()
        == DomainValidationCandidateStatus::Validated
        && candidate.validation().is_some()
        && candidate.failure().is_none();
    let outcome = request.finish(
        provider.descriptor(),
        candidate,
        InstallationOutcome::Current,
    );
    let Some(receipt) = outcome.receipt() else { return false };
    candidate_fields
        && outcome.status() == DomainValidationStatus::Validated
        && outcome.failure().is_none()
        && outcome.violation().is_none()
        && receipt.matches_intent(&fixture.domain, &intent)
        && receipt.stamp().request().value() == 1_061
}

fn semantic_rejection() -> bool {
    let Some(fixture) = support::fixture() else { return false };
    let Some(intent) = support::make_intent(&fixture, b"invalid") else {
        return false;
    };
    let Some(request) = request(&fixture, &intent) else { return false };
    let inspect =
        ScriptedDomain { descriptor: &fixture.domain, script: Script::Inspect };
    let invalid = request.finish(
        inspect.descriptor(),
        invoke(&inspect, request),
        InstallationOutcome::Current,
    );
    let provider_failure = ScriptedDomain {
        descriptor: &fixture.domain,
        script: Script::Reject(SemanticPolicyFailure::ProviderFailure),
    };
    let failed = request.finish(
        provider_failure.descriptor(),
        invoke(&provider_failure, request),
        InstallationOutcome::Current,
    );
    invalid.status() == DomainValidationStatus::Rejected
        && invalid.failure() == Some(SemanticPolicyFailure::InvalidIntent)
        && invalid.receipt().is_none()
        && invalid.violation().is_none()
        && failed.failure() == Some(SemanticPolicyFailure::ProviderFailure)
}

fn wrong_candidate() -> bool {
    let Some(fixture) = support::fixture() else { return false };
    let Some(intent) = support::make_intent(&fixture, support::INTENT_BYTES)
    else {
        return false;
    };
    let Some(request) = request(&fixture, &intent) else { return false };
    let provider = ScriptedDomain {
        descriptor: &fixture.domain,
        script: Script::WrongIntent,
    };
    let outcome = request.finish(
        provider.descriptor(),
        invoke(&provider, request),
        InstallationOutcome::Current,
    );
    outcome.status() == DomainValidationStatus::ContractViolation
        && outcome.violation()
            == Some(SemanticPolicyContractError::ValidationStampMismatch)
        && outcome.receipt().is_none()
        && outcome.failure().is_none()
}

fn wrong_provider() -> bool {
    let Some(fixture) = support::fixture() else { return false };
    let Some(intent) = support::make_intent(&fixture, support::INTENT_BYTES)
    else {
        return false;
    };
    let Some(request) = request(&fixture, &intent) else { return false };
    let Some(other) = support::domain_descriptor(2) else { return false };
    let outcome = request.finish(
        &other,
        request.validated_candidate(),
        InstallationOutcome::Current,
    );
    outcome.violation()
        == Some(SemanticPolicyContractError::ProviderDescriptorMismatch)
}

fn post_callback_fence() -> bool {
    let Some(fixture) = support::fixture() else { return false };
    let Some(intent) = support::make_intent(&fixture, support::INTENT_BYTES)
    else {
        return false;
    };
    let Some(request) = request(&fixture, &intent) else { return false };
    let outcome = request.finish(
        &fixture.domain,
        request.validated_candidate(),
        InstallationOutcome::Fenced(FenceCause::Replacement),
    );
    outcome.status() == DomainValidationStatus::Rejected
        && outcome.failure() == Some(SemanticPolicyFailure::Replaced)
        && outcome.receipt().is_none()
        && outcome.violation().is_none()
}

fn request_failures() -> bool {
    let Some(fixture) = support::fixture() else { return false };
    let Some(intent) = support::make_intent(&fixture, support::INTENT_BYTES)
    else {
        return false;
    };
    cancelled(&fixture, &intent)
        && deadline(&fixture, &intent)
        && wrong_charge_owner(&fixture, &intent)
        && excessive_charge(&fixture, &intent)
        && wrong_domain(&fixture, &intent)
}

fn cancelled(
    fixture: &SemanticFixture,
    intent: &reovim_kernel_interface::semantic::intent::SemanticIntent,
) -> bool {
    let Some(charge) = charge(fixture) else { return false };
    let code = match DomainValidationRequest::new(
        &fixture.domain,
        &fixture.representation,
        support::input(fixture, intent),
        charge,
        CancellationState::Requested(CancellationCause::ExplicitRequest),
        DeadlineState::Pending,
    ) {
        Err(SemanticPolicyRequestError::Cancelled(
            CancellationCause::ExplicitRequest,
        )) => 0,
        _ => 1,
    };
    code == 0
}

fn deadline(
    fixture: &SemanticFixture,
    intent: &reovim_kernel_interface::semantic::intent::SemanticIntent,
) -> bool {
    let Some(charge) = charge(fixture) else { return false };
    let code = match DomainValidationRequest::new(
        &fixture.domain,
        &fixture.representation,
        support::input(fixture, intent),
        charge,
        CancellationState::Open,
        DeadlineState::Reached,
    ) {
        Err(SemanticPolicyRequestError::DeadlineReached) => 0,
        _ => 1,
    };
    code == 0
}

fn wrong_charge_owner(
    fixture: &SemanticFixture,
    intent: &reovim_kernel_interface::semantic::intent::SemanticIntent,
) -> bool {
    let Some(bounds) = support::call_bounds(9_999, 16) else {
        return false;
    };
    let Some(charge) = bounds.prepare(4, 0, 0, 1).ok() else {
        return false;
    };
    let code = match DomainValidationRequest::new(
            &fixture.domain,
            &fixture.representation,
            support::input(fixture, intent),
            charge,
            CancellationState::Open,
            DeadlineState::Pending,
        ) {
        Err(SemanticPolicyRequestError::Bound(
            reovim_kernel_interface::semantic::bounds::
                SemanticBoundViolation::ChargeOwnerMismatch { .. },
        )) => 0,
        _ => 1,
    };
    code == 0
}

fn excessive_charge(
    fixture: &SemanticFixture,
    intent: &reovim_kernel_interface::semantic::intent::SemanticIntent,
) -> bool {
    let Some(bounds) = support::call_bounds(support::DOMAIN_CALL_OWNER, 32)
    else {
        return false;
    };
    let Some(charge) = bounds.prepare(17, 0, 0, 1).ok() else {
        return false;
    };
    let code = match DomainValidationRequest::new(
            &fixture.domain,
            &fixture.representation,
            support::input(fixture, intent),
            charge,
            CancellationState::Open,
            DeadlineState::Pending,
        ) {
        Err(SemanticPolicyRequestError::Bound(
            reovim_kernel_interface::semantic::bounds::
                SemanticBoundViolation::Bound {
                    field: reovim_kernel_interface::semantic::bounds::
                        SemanticBoundField::PolicyWork,
                    ..
                },
        )) => 0,
        _ => 1,
    };
    code == 0
}

fn wrong_domain(
    fixture: &SemanticFixture,
    intent: &reovim_kernel_interface::semantic::intent::SemanticIntent,
) -> bool {
    let Some(other) = support::domain_descriptor(2) else { return false };
    let Some(charge) = charge(fixture) else { return false };
    let code = match DomainValidationRequest::new(
        &other,
        &fixture.representation,
        support::input(fixture, intent),
        charge,
        CancellationState::Open,
        DeadlineState::Pending,
    ) {
        Err(SemanticPolicyRequestError::Contract(
            SemanticPolicyContractError::IntentDomainMismatch,
        )) => 0,
        _ => 1,
    };
    code == 0
}

fn request<'a>(
    fixture: &'a SemanticFixture,
    intent: &'a reovim_kernel_interface::semantic::intent::SemanticIntent,
) -> Option<DomainValidationRequest<'a, ()>> {
    DomainValidationRequest::new(
        &fixture.domain,
        &fixture.representation,
        support::input(fixture, intent),
        charge(fixture)?,
        CancellationState::Open,
        DeadlineState::Pending,
    )
    .ok()
}

fn charge(
    fixture: &SemanticFixture,
) -> Option<reovim_kernel_interface::semantic::bounds::SemanticPolicyCallCharge>
{
    fixture.domain.validation().bounds().prepare(4, 0, 0, 1).ok()
}

fn invoke(
    provider: &dyn DomainPolicyPort<State = ()>,
    request: DomainValidationRequest<'_, ()>,
) -> DomainValidationCandidate {
    provider.validate(request)
}

const fn wrong_intent_candidate(
    request: DomainValidationRequest<'_, ()>,
) -> DomainValidationCandidate {
    let stamp = request.validation_stamp();
    let original = request.intent().stamp();
    let altered =
        reovim_kernel_interface::semantic::stamp::SemanticIntentStamp::new(
            SemanticIntentIdentity::new(9_999),
            original.caller(),
            original.request(),
            *original.target(),
            *original.plan(),
            *original.projection(),
            original.coordinate(),
        );
    DomainValidationCandidate::validated(SemanticIntentValidationStamp::new(
        stamp.domain(),
        &altered,
        stamp.carrier(),
        stamp.shape(),
        request.domain().state_model(),
    ))
}

fn failure_vocabulary() -> bool {
    let failures = [
        SemanticPolicyFailure::UnsupportedCapability,
        SemanticPolicyFailure::UnsupportedSchema,
        SemanticPolicyFailure::Unrepresentable,
        SemanticPolicyFailure::StateUnavailable,
        SemanticPolicyFailure::InvalidIntent,
        SemanticPolicyFailure::NoCorrespondence,
        SemanticPolicyFailure::AmbiguousCorrespondence,
        SemanticPolicyFailure::StaleState,
        SemanticPolicyFailure::ReadOnly,
        SemanticPolicyFailure::Pressure,
        SemanticPolicyFailure::Cancelled,
        SemanticPolicyFailure::DeadlineReached,
        SemanticPolicyFailure::CallerTimedOut,
        SemanticPolicyFailure::Replaced,
        SemanticPolicyFailure::Closing,
        SemanticPolicyFailure::Closed,
        SemanticPolicyFailure::ProviderFailure,
        SemanticPolicyFailure::ProviderPanicked,
    ];
    let mappings = SemanticPolicyFailure::from_fence(FenceCause::Cancellation)
        == SemanticPolicyFailure::Cancelled
        && SemanticPolicyFailure::from_fence(FenceCause::Deadline)
            == SemanticPolicyFailure::DeadlineReached
        && SemanticPolicyFailure::from_fence(FenceCause::CallerTimeout)
            == SemanticPolicyFailure::CallerTimedOut
        && SemanticPolicyFailure::from_fence(FenceCause::Replacement)
            == SemanticPolicyFailure::Replaced
        && SemanticPolicyFailure::from_fence(FenceCause::Closing)
            == SemanticPolicyFailure::Closing
        && SemanticPolicyFailure::from_fence(FenceCause::Closed)
            == SemanticPolicyFailure::Closed;
    mappings
        && failures.into_iter().all(|failure| crate::error_traits(&failure))
}
