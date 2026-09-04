mod faults;

use reovim_kernel_interface::buffer::BufferStateStamp;
use reovim_kernel_interface::buffer::carrier::BufferCarrierRole;
use reovim_kernel_interface::buffer::plan::{
    BufferPlanBindingOutcome, BufferPlanDescriptor, BufferPlanStamp,
    BufferTransactionIdentity,
};
use reovim_kernel_interface::cancellation::{
    CancellationState, DeadlineState, InstallationOutcome,
};
use reovim_kernel_interface::semantic::domain::DomainValidationReceipt;
use reovim_kernel_interface::semantic::policy::SemanticPolicyFailure;
use reovim_kernel_interface::semantic::representation::{
    RepresentationLoweringCandidate, RepresentationLoweringCandidateStatus,
    RepresentationLoweringRequest, RepresentationLoweringStatus,
    RepresentationPolicyPort,
};

use super::support::{self, SemanticFixture};

#[derive(Clone, Copy)]
pub(super) enum Script {
    Lower,
    Reject(SemanticPolicyFailure),
    WrongStamp,
    WrongTarget,
    WrongCarrier,
}

pub(super) struct ScriptedRepresentation<'a> {
    pub descriptor: &'a reovim_kernel_interface::semantic::descriptor::
        RepresentationPolicyDescriptor,
    pub script: Script,
}

impl ::reovim_kernel_interface::semantic::representation::
    RepresentationPolicyPort for ScriptedRepresentation<'_>
{
    fn descriptor(
        &self,
    ) -> &reovim_kernel_interface::semantic::descriptor::
    RepresentationPolicyDescriptor{
        self.descriptor
    }

    fn lower(
        &self,
        request: RepresentationLoweringRequest<'_>,
    ) -> RepresentationLoweringCandidate {
        match self.script {
            Script::Lower => exact_plan(request),
            Script::Reject(failure) => {
                RepresentationLoweringCandidate::rejected(failure)
            }
            Script::WrongStamp => wrong_stamp_plan(request),
            Script::WrongTarget => wrong_target_plan(request),
            Script::WrongCarrier => wrong_carrier_plan(request),
        }
    }
}

pub const fn const_cases() -> bool {
    crate::derive_witness::<
        ::reovim_kernel_interface::semantic::representation::
            RepresentationLoweringCandidateStatus,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::semantic::representation::
            RepresentationLoweringStatus,
    >() && crate::clone_copy_witness::<
        ::reovim_kernel_interface::semantic::representation::
            RepresentationLoweringRequest<'static>,
    >() && crate::send_sync_witness::<
        ::reovim_kernel_interface::semantic::representation::
            RepresentationLoweringCandidate,
    >() && crate::send_sync_witness::<
        ::reovim_kernel_interface::semantic::representation::
            RepresentationLoweringOutcome,
    >()
}

pub fn exercise() -> bool {
    valid_lowering() && expected_rejection() && faults::exercise()
}

fn valid_lowering() -> bool {
    let Some(fixture) = support::fixture() else { return false };
    let Some(intent) = support::make_intent(&fixture, support::INTENT_BYTES)
    else {
        return false;
    };
    let Some(receipt) = super::domain::validated_receipt(&fixture, &intent)
    else {
        return false;
    };
    let Some(request) = request(&fixture, &intent, &receipt) else {
        return false;
    };
    let provider = ScriptedRepresentation {
        descriptor: &fixture.representation,
        script: Script::Lower,
    };
    let candidate = invoke(&provider, request);
    let Some(plan) = candidate.plan() else { return false };
    crate::require_debug(&candidate);
    let candidate_fields = candidate.status()
        == RepresentationLoweringCandidateStatus::Lowered
        && candidate.failure().is_none()
        && plan.stamp().same_plan(*intent.stamp().plan())
        && plan.target().same_state(*intent.stamp().target())
        && fixture.buffer.validate_plan(plan).is_ok();
    let outcome = candidate.finish(
        request,
        provider.descriptor(),
        InstallationOutcome::Current,
    );
    let Some(plan) = outcome.plan() else { return false };
    crate::require_debug(&outcome);
    let outcome_fields = outcome.status()
        == RepresentationLoweringStatus::Lowered
        && outcome.failure().is_none()
        && outcome.violation().is_none()
        && plan.origin().bytes() == b"alpha"
        && plan.operations().bytes() == b"bravo"
        && plan.preconditions().bytes().is_empty();
    let Some(plan) = outcome.into_plan() else { return false };
    candidate_fields
        && outcome_fields
        && plan.stamp().same_plan(*intent.stamp().plan())
}

fn expected_rejection() -> bool {
    let Some(fixture) = support::fixture() else { return false };
    let Some(intent) = support::make_intent(&fixture, support::INTENT_BYTES)
    else {
        return false;
    };
    let Some(receipt) = super::domain::validated_receipt(&fixture, &intent)
    else {
        return false;
    };
    let Some(request) = request(&fixture, &intent, &receipt) else {
        return false;
    };
    let provider = ScriptedRepresentation {
        descriptor: &fixture.representation,
        script: Script::Reject(SemanticPolicyFailure::NoCorrespondence),
    };
    let candidate = invoke(&provider, request);
    let candidate_fields = candidate.status()
        == RepresentationLoweringCandidateStatus::Rejected
        && candidate.plan().is_none()
        && candidate.failure() == Some(SemanticPolicyFailure::NoCorrespondence);
    let outcome = candidate.finish(
        request,
        provider.descriptor(),
        InstallationOutcome::Current,
    );
    candidate_fields
        && outcome.status() == RepresentationLoweringStatus::Rejected
        && outcome.failure() == Some(SemanticPolicyFailure::NoCorrespondence)
        && outcome.plan().is_none()
        && outcome.violation().is_none()
}

pub(super) fn request<'a>(
    fixture: &'a SemanticFixture,
    intent: &'a reovim_kernel_interface::semantic::intent::SemanticIntent,
    receipt: &'a DomainValidationReceipt,
) -> Option<RepresentationLoweringRequest<'a>> {
    RepresentationLoweringRequest::new(
        &fixture.representation,
        intent,
        receipt,
        &fixture.buffer,
        charge(fixture)?,
        CancellationState::Open,
        DeadlineState::Pending,
    )
    .ok()
}

pub(super) fn charge(
    fixture: &SemanticFixture,
) -> Option<reovim_kernel_interface::semantic::bounds::SemanticPolicyCallCharge>
{
    fixture.representation.lowering().bounds().prepare(6, 0, 20, 1).ok()
}

pub(super) fn invoke(
    provider: &dyn RepresentationPolicyPort,
    request: RepresentationLoweringRequest<'_>,
) -> RepresentationLoweringCandidate {
    provider.lower(request)
}

fn exact_plan(
    request: RepresentationLoweringRequest<'_>,
) -> RepresentationLoweringCandidate {
    build_plan(
        request.plan(),
        *request.intent().stamp().plan(),
        request.intent().stamp().target(),
    )
}

fn wrong_stamp_plan(
    request: RepresentationLoweringRequest<'_>,
) -> RepresentationLoweringCandidate {
    let exact = *request.intent().stamp().plan();
    let wrong = BufferPlanStamp::new(
        exact.owner(),
        BufferTransactionIdentity::new(9_999),
        exact.history_transition(),
        exact.cancellation(),
        exact.correlation(),
        exact.deadline(),
    );
    build_plan(request.plan(), wrong, request.intent().stamp().target())
}

fn wrong_target_plan(
    request: RepresentationLoweringRequest<'_>,
) -> RepresentationLoweringCandidate {
    let Some(target) = crate::buffer::state(1, 1, 0) else {
        return RepresentationLoweringCandidate::rejected(
            SemanticPolicyFailure::ProviderFailure,
        );
    };
    build_plan(request.plan(), *request.intent().stamp().plan(), &target)
}

fn wrong_carrier_plan(
    request: RepresentationLoweringRequest<'_>,
) -> RepresentationLoweringCandidate {
    let Some(descriptor) = support::alternate_buffer_descriptor() else {
        return RepresentationLoweringCandidate::rejected(
            SemanticPolicyFailure::ProviderFailure,
        );
    };
    build_plan(
        &descriptor,
        *request.intent().stamp().plan(),
        request.intent().stamp().target(),
    )
}

fn build_plan(
    descriptor: &BufferPlanDescriptor,
    stamp: BufferPlanStamp,
    target: &BufferStateStamp,
) -> RepresentationLoweringCandidate {
    let Some(origin) = crate::buffer::carrier::make_carrier(
        descriptor,
        BufferCarrierRole::Origin,
        1,
        b"alpha",
    ) else {
        return provider_failure();
    };
    let Some(operations) = crate::buffer::carrier::make_carrier(
        descriptor,
        BufferCarrierRole::Operations,
        1,
        b"bravo",
    ) else {
        return provider_failure();
    };
    let Some(preconditions) = crate::buffer::carrier::make_carrier(
        descriptor,
        BufferCarrierRole::Preconditions,
        0,
        b"",
    ) else {
        return provider_failure();
    };
    match descriptor.bind_plan(
        stamp,
        *target,
        origin,
        operations,
        preconditions,
    ) {
        BufferPlanBindingOutcome::Accepted(plan) => plan.into(),
        BufferPlanBindingOutcome::Rejected(_) => provider_failure(),
    }
}

const fn provider_failure() -> RepresentationLoweringCandidate {
    RepresentationLoweringCandidate::rejected(
        SemanticPolicyFailure::ProviderFailure,
    )
}
