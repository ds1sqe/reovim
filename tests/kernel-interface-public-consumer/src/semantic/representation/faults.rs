use reovim_kernel_interface::buffer::carrier::{
    BufferCarrierError, BufferCarrierRole,
};
use reovim_kernel_interface::buffer::plan::BufferPlanError;
use reovim_kernel_interface::cancellation::{
    CancellationCause, CancellationState, DeadlineState, FenceCause,
    InstallationOutcome,
};
use reovim_kernel_interface::semantic::policy::{
    SemanticPolicyContractError, SemanticPolicyFailure,
    SemanticPolicyRequestError,
};
use reovim_kernel_interface::semantic::representation::{
    RepresentationLoweringRequest, RepresentationLoweringStatus,
    RepresentationPolicyPort,
};

use super::{Script, ScriptedRepresentation};
use crate::semantic::support::{self, SemanticFixture};

pub fn exercise() -> bool {
    wrong_receipt()
        && read_only()
        && wrong_representation()
        && wrong_plan_descriptors()
        && request_state_failures()
        && wrong_provider()
        && returned_plan_failures()
}

fn wrong_receipt() -> bool {
    let Some(fixture) = support::fixture() else { return false };
    let Some(first) = support::make_intent(&fixture, support::INTENT_BYTES)
    else {
        return false;
    };
    let Some(second) = support::make_intent_with(
        &fixture,
        support::INTENT_BYTES,
        2_060,
        2_061,
    ) else {
        return false;
    };
    let Some(receipt) =
        crate::semantic::domain::validated_receipt(&fixture, &first)
    else {
        return false;
    };
    let Some(charge) = super::charge(&fixture) else { return false };
    let code = match RepresentationLoweringRequest::new(
        &fixture.representation,
        &second,
        &receipt,
        &fixture.buffer,
        charge,
        CancellationState::Open,
        DeadlineState::Pending,
    ) {
        Err(SemanticPolicyRequestError::Contract(
            SemanticPolicyContractError::ValidationReceiptMismatch,
        )) => 0,
        _ => 1,
    };
    code == 0
}

fn read_only() -> bool {
    let Some(fixture) = support::fixture() else { return false };
    let Some(intent) = support::make_intent(&fixture, support::INTENT_BYTES)
    else {
        return false;
    };
    let Some(receipt) =
        crate::semantic::domain::validated_receipt(&fixture, &intent)
    else {
        return false;
    };
    let Some(read_only) = support::read_only_representation(&fixture) else {
        return false;
    };
    let Some(charge) = super::charge(&fixture) else { return false };
    let code = match RepresentationLoweringRequest::new(
        &read_only,
        &intent,
        &receipt,
        &fixture.buffer,
        charge,
        CancellationState::Open,
        DeadlineState::Pending,
    ) {
        Err(SemanticPolicyRequestError::ReadOnly) => 0,
        _ => 1,
    };
    code == 0
}

fn wrong_representation() -> bool {
    let Some(fixture) = support::fixture() else { return false };
    let Some(intent) = support::make_intent(&fixture, support::INTENT_BYTES)
    else {
        return false;
    };
    let Some(receipt) =
        crate::semantic::domain::validated_receipt(&fixture, &intent)
    else {
        return false;
    };
    let Some(replacement) = support::replacement_representation(&fixture)
    else {
        return false;
    };
    let Some(charge) = super::charge(&fixture) else { return false };
    let code = match RepresentationLoweringRequest::new(
        &replacement,
        &intent,
        &receipt,
        &fixture.buffer,
        charge,
        CancellationState::Open,
        DeadlineState::Pending,
    ) {
        Err(SemanticPolicyRequestError::Contract(
            SemanticPolicyContractError::IntentRepresentationMismatch,
        )) => 0,
        _ => 1,
    };
    code == 0
}

fn wrong_plan_descriptors() -> bool {
    let Some(fixture) = support::fixture() else { return false };
    let Some(intent) = support::make_intent(&fixture, support::INTENT_BYTES)
    else {
        return false;
    };
    let Some(receipt) =
        crate::semantic::domain::validated_receipt(&fixture, &intent)
    else {
        return false;
    };
    plan_contract_mismatch(&fixture, &intent, &receipt)
        && plan_owner_mismatch(&fixture, &intent, &receipt)
}

fn plan_contract_mismatch(
    fixture: &SemanticFixture,
    intent: &reovim_kernel_interface::semantic::intent::SemanticIntent,
    receipt: &reovim_kernel_interface::semantic::domain::
        DomainValidationReceipt,
) -> bool {
    let Some(plan) = support::buffer_descriptor(9_999, 1) else {
        return false;
    };
    request_with_plan(fixture, intent, receipt, &plan)
        == Some(SemanticPolicyContractError::PlanDescriptorMismatch)
}

fn plan_owner_mismatch(
    fixture: &SemanticFixture,
    intent: &reovim_kernel_interface::semantic::intent::SemanticIntent,
    receipt: &reovim_kernel_interface::semantic::domain::
        DomainValidationReceipt,
) -> bool {
    let Some(plan) = support::buffer_descriptor(800, 2) else {
        return false;
    };
    request_with_plan(fixture, intent, receipt, &plan)
        == Some(SemanticPolicyContractError::PlanOwnerMismatch)
}

fn request_with_plan(
    fixture: &SemanticFixture,
    intent: &reovim_kernel_interface::semantic::intent::SemanticIntent,
    receipt: &reovim_kernel_interface::semantic::domain::
        DomainValidationReceipt,
    plan: &reovim_kernel_interface::buffer::plan::BufferPlanDescriptor,
) -> Option<SemanticPolicyContractError> {
    let charge = super::charge(fixture)?;
    match RepresentationLoweringRequest::new(
        &fixture.representation,
        intent,
        receipt,
        plan,
        charge,
        CancellationState::Open,
        DeadlineState::Pending,
    ) {
        Err(SemanticPolicyRequestError::Contract(error)) => Some(error),
        _ => None,
    }
}

fn request_state_failures() -> bool {
    let Some(fixture) = support::fixture() else { return false };
    let Some(intent) = support::make_intent(&fixture, support::INTENT_BYTES)
    else {
        return false;
    };
    let Some(receipt) =
        crate::semantic::domain::validated_receipt(&fixture, &intent)
    else {
        return false;
    };
    cancelled(&fixture, &intent, &receipt)
        && deadline(&fixture, &intent, &receipt)
        && wrong_charge(&fixture, &intent, &receipt)
        && excessive_charge(&fixture, &intent, &receipt)
}

fn cancelled(
    fixture: &SemanticFixture,
    intent: &reovim_kernel_interface::semantic::intent::SemanticIntent,
    receipt: &reovim_kernel_interface::semantic::domain::
        DomainValidationReceipt,
) -> bool {
    let Some(charge) = super::charge(fixture) else { return false };
    let code = match RepresentationLoweringRequest::new(
        &fixture.representation,
        intent,
        receipt,
        &fixture.buffer,
        charge,
        CancellationState::Requested(CancellationCause::Replacement),
        DeadlineState::Pending,
    ) {
        Err(SemanticPolicyRequestError::Cancelled(
            CancellationCause::Replacement,
        )) => 0,
        _ => 1,
    };
    code == 0
}

fn deadline(
    fixture: &SemanticFixture,
    intent: &reovim_kernel_interface::semantic::intent::SemanticIntent,
    receipt: &reovim_kernel_interface::semantic::domain::
        DomainValidationReceipt,
) -> bool {
    let Some(charge) = super::charge(fixture) else { return false };
    let code = match RepresentationLoweringRequest::new(
        &fixture.representation,
        intent,
        receipt,
        &fixture.buffer,
        charge,
        CancellationState::Open,
        DeadlineState::Reached,
    ) {
        Err(SemanticPolicyRequestError::DeadlineReached) => 0,
        _ => 1,
    };
    code == 0
}

fn wrong_charge(
    fixture: &SemanticFixture,
    intent: &reovim_kernel_interface::semantic::intent::SemanticIntent,
    receipt: &reovim_kernel_interface::semantic::domain::
        DomainValidationReceipt,
) -> bool {
    let Some(bounds) = support::call_bounds(9_999, 24) else {
        return false;
    };
    let Some(charge) = bounds.prepare(6, 0, 20, 1).ok() else {
        return false;
    };
    let code = match RepresentationLoweringRequest::new(
            &fixture.representation,
            intent,
            receipt,
            &fixture.buffer,
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
    receipt: &reovim_kernel_interface::semantic::domain::
        DomainValidationReceipt,
) -> bool {
    let Some(bounds) =
        support::call_bounds(support::REPRESENTATION_CALL_OWNER, 48)
    else {
        return false;
    };
    let Some(charge) = bounds.prepare(25, 0, 20, 1).ok() else {
        return false;
    };
    let code = match RepresentationLoweringRequest::new(
            &fixture.representation,
            intent,
            receipt,
            &fixture.buffer,
            charge,
            CancellationState::Open,
            DeadlineState::Pending,
        ) {
        Err(SemanticPolicyRequestError::Bound(
            reovim_kernel_interface::semantic::bounds::
                SemanticBoundViolation::Bound { .. },
        )) => 0,
        _ => 1,
    };
    code == 0
}

fn wrong_provider() -> bool {
    let Some(fixture) = support::fixture() else { return false };
    let Some(intent) = support::make_intent(&fixture, support::INTENT_BYTES)
    else {
        return false;
    };
    let Some(receipt) =
        crate::semantic::domain::validated_receipt(&fixture, &intent)
    else {
        return false;
    };
    let Some(request) = super::request(&fixture, &intent, &receipt) else {
        return false;
    };
    let provider = ScriptedRepresentation {
        descriptor: &fixture.representation,
        script: Script::Lower,
    };
    let Some(replacement) = support::replacement_representation(&fixture)
    else {
        return false;
    };
    let outcome = super::invoke(&provider, request).finish(
        request,
        &replacement,
        InstallationOutcome::Current,
    );
    outcome.status() == RepresentationLoweringStatus::ContractViolation
        && outcome.violation()
            == Some(SemanticPolicyContractError::ProviderDescriptorMismatch)
        && outcome.plan().is_some()
}

fn returned_plan_failures() -> bool {
    returned_plan_failure(
        Script::WrongStamp,
        SemanticPolicyContractError::ReturnedPlanStampMismatch,
    ) && returned_plan_failure(
        Script::WrongTarget,
        SemanticPolicyContractError::ReturnedPlanTargetMismatch,
    ) && returned_charge_failure()
        && post_callback_fence()
        && returned_carrier_failure()
}

fn post_callback_fence() -> bool {
    let Some(fixture) = support::fixture() else { return false };
    let Some(intent) = support::make_intent(&fixture, support::INTENT_BYTES)
    else {
        return false;
    };
    let Some(receipt) =
        crate::semantic::domain::validated_receipt(&fixture, &intent)
    else {
        return false;
    };
    let Some(request) = super::request(&fixture, &intent, &receipt) else {
        return false;
    };
    let provider = ScriptedRepresentation {
        descriptor: &fixture.representation,
        script: Script::Lower,
    };
    let outcome = super::invoke(&provider, request).finish(
        request,
        provider.descriptor(),
        InstallationOutcome::Fenced(FenceCause::CallerTimeout),
    );
    outcome.status() == RepresentationLoweringStatus::Rejected
        && outcome.failure() == Some(SemanticPolicyFailure::CallerTimedOut)
        && outcome.violation().is_none()
        && outcome.plan().is_some()
}

fn returned_charge_failure() -> bool {
    let Some(fixture) = support::fixture() else { return false };
    let Some(intent) = support::make_intent(&fixture, support::INTENT_BYTES)
    else {
        return false;
    };
    let Some(receipt) =
        crate::semantic::domain::validated_receipt(&fixture, &intent)
    else {
        return false;
    };
    let Some(charge) =
        fixture.representation.lowering().bounds().prepare(6, 0, 5, 1).ok()
    else {
        return false;
    };
    let Some(request) = RepresentationLoweringRequest::new(
        &fixture.representation,
        &intent,
        &receipt,
        &fixture.buffer,
        charge,
        CancellationState::Open,
        DeadlineState::Pending,
    )
    .ok() else {
        return false;
    };
    let provider = ScriptedRepresentation {
        descriptor: &fixture.representation,
        script: Script::Lower,
    };
    let outcome = super::invoke(&provider, request).finish(
        request,
        provider.descriptor(),
        InstallationOutcome::Current,
    );
    outcome.violation()
        == Some(SemanticPolicyContractError::ReturnedPlanChargeExceeded {
            reserved: 5,
            actual: 10,
        })
        && outcome.plan().is_some()
}

fn returned_plan_failure(
    script: Script,
    expected: SemanticPolicyContractError,
) -> bool {
    let Some(fixture) = support::fixture() else { return false };
    let Some(intent) = support::make_intent(&fixture, support::INTENT_BYTES)
    else {
        return false;
    };
    let Some(receipt) =
        crate::semantic::domain::validated_receipt(&fixture, &intent)
    else {
        return false;
    };
    let Some(request) = super::request(&fixture, &intent, &receipt) else {
        return false;
    };
    let provider =
        ScriptedRepresentation { descriptor: &fixture.representation, script };
    let outcome = super::invoke(&provider, request).finish(
        request,
        provider.descriptor(),
        InstallationOutcome::Current,
    );
    outcome.status() == RepresentationLoweringStatus::ContractViolation
        && outcome.violation() == Some(expected)
        && outcome.plan().is_some()
}

fn returned_carrier_failure() -> bool {
    let Some(fixture) = support::fixture() else { return false };
    let Some(intent) = support::make_intent(&fixture, support::INTENT_BYTES)
    else {
        return false;
    };
    let Some(receipt) =
        crate::semantic::domain::validated_receipt(&fixture, &intent)
    else {
        return false;
    };
    let Some(request) = super::request(&fixture, &intent, &receipt) else {
        return false;
    };
    let provider = ScriptedRepresentation {
        descriptor: &fixture.representation,
        script: Script::WrongCarrier,
    };
    let outcome = super::invoke(&provider, request).finish(
        request,
        provider.descriptor(),
        InstallationOutcome::Current,
    );
    let violation = outcome.violation();
    let violation_code = match violation {
        Some(SemanticPolicyContractError::ReturnedPlanInvalid(
            BufferPlanError::Carrier(BufferCarrierError::TypeMismatch {
                role: BufferCarrierRole::Origin,
            }),
        )) => 0,
        _ => 1,
    };
    violation_code == 0
        && outcome.plan().is_some()
        && outcome.into_plan().is_some()
}
