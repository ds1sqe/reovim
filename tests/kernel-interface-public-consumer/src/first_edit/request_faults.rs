use reovim_kernel_interface::bound::ZeroPolicy;
use reovim_kernel_interface::buffer::{BufferStateStamp, BufferVersion};
use reovim_kernel_interface::cancellation::{
    CancellationState, DeadlineState, FenceCause, InstallationOutcome,
};
use reovim_kernel_interface::resource::state::ResourceStateModel;
use reovim_kernel_interface::semantic::bounds::{
    SemanticBoundField, SemanticBoundViolation, SemanticIntentBounds,
};
use reovim_kernel_interface::semantic::domain::DomainValidationRequest;
use reovim_kernel_interface::semantic::intent::SemanticIntentError;
use reovim_kernel_interface::semantic::policy::{
    SemanticPolicyContractError, SemanticPolicyFailure,
    SemanticPolicyRequestError,
};
use reovim_kernel_interface::semantic::{
    descriptor::SemanticCoordinateSpaceDescriptor,
    representation::RepresentationLoweringRequest, state::SemanticPolicyInput,
};

use super::native::{models::ByteModel, support as native};
use super::{policies::Policies, receipt_faults};
use crate::semantic::support;

/// Exercises bound actual-state input and late semantic installation fences.
pub fn exercise() {
    observed_state_mismatch();
    selected_carrier_and_bounds();
    late_lowering_fence();
}

fn observed_state_mismatch() {
    let model = ByteModel::new();
    let fixture =
        support::fixture_for_model(&model.binding()).expect("fixture");
    let data = b"# a\n\xff".to_vec();
    let intent = support::make_intent(&fixture, b"b").expect("intent");
    let mut other_model = model.binding();
    other_model.contract = native::selection(999);
    let stamp = fixture.state.stamp;
    let advanced = BufferStateStamp::new(
        stamp.source(),
        stamp.buffer(),
        BufferVersion::new(1),
        stamp.history(),
    );
    let cases = [
        (
            native::view(&other_model, &stamp, &data),
            SemanticPolicyContractError::WorkingStateModelMismatch,
        ),
        (
            native::view(&model.binding(), &advanced, &data),
            SemanticPolicyContractError::WorkingStateTargetMismatch,
        ),
    ];
    for (observed, cause) in cases {
        let charge = fixture
            .domain
            .validation()
            .bounds()
            .prepare(8, 16, 0, 1)
            .expect("Domain reservation");
        let result = DomainValidationRequest::new(
            &fixture.domain,
            &fixture.representation,
            SemanticPolicyInput { intent: &intent, state: &observed },
            charge,
            CancellationState::Open,
            DeadlineState::Pending,
        );
        assert_eq!(
            result.err(),
            Some(SemanticPolicyRequestError::Contract(cause))
        );
        assert_eq!(data, b"# a\n\xff");
    }
}

fn selected_carrier_and_bounds() {
    let model = ByteModel::new();
    let fixture =
        support::fixture_for_model(&model.binding()).expect("fixture");
    let data = b"# a\n\xff".to_vec();
    let state = native::view(&model.binding(), &fixture.state.stamp, &data);
    let intent = support::make_intent(&fixture, b"bb").expect("original bound");
    let alternate =
        support::intent_contract_for(999).expect("alternate carrier");
    let descriptor = receipt_faults::rebuild(
        &fixture,
        fixture.representation.coordinate(),
        &alternate,
    );
    let charge = fixture
        .domain
        .validation()
        .bounds()
        .prepare(8, 16, 0, 1)
        .expect("Domain reservation");
    let result = DomainValidationRequest::new(
        &fixture.domain,
        &descriptor,
        SemanticPolicyInput { intent: &intent, state: &state },
        charge,
        CancellationState::Open,
        DeadlineState::Pending,
    );
    assert_eq!(
        result.err(),
        Some(SemanticPolicyRequestError::Contract(
            SemanticPolicyContractError::RetainedIntentInvalid(
                SemanticIntentError::TypeMismatch
            )
        ))
    );

    let coordinate = fixture.representation.coordinate();
    let bounds = coordinate.intent_bounds();
    let tight = SemanticIntentBounds::new(
        bounds.charge_owner(),
        bounds.items(),
        bounds.depth(),
        support::byte_bound(
            bounds.charge_owner().value(),
            1,
            ZeroPolicy::Reject,
        )
        .expect("one-byte selected intent bound"),
    )
    .expect("coherent tightened descriptor");
    let coordinate = SemanticCoordinateSpaceDescriptor::new(
        coordinate.stamp(),
        coordinate.coordinate_schema(),
        coordinate.region_schema(),
        coordinate.value_schema(),
        &tight,
    );
    let descriptor = receipt_faults::rebuild(
        &fixture,
        &coordinate,
        fixture.representation.intent(),
    );
    let result = DomainValidationRequest::new(
        &fixture.domain,
        &descriptor,
        SemanticPolicyInput { intent: &intent, state: &state },
        charge,
        CancellationState::Open,
        DeadlineState::Pending,
    );
    assert!(matches!(
        result.err(),
        Some(SemanticPolicyRequestError::Contract(
            SemanticPolicyContractError::RetainedIntentInvalid(
                SemanticIntentError::Bound(SemanticBoundViolation::Bound {
                    field: SemanticBoundField::IntentBytes,
                    ..
                })
            )
        ))
    ));
    assert_eq!(data, b"# a\n\xff");
}

fn late_lowering_fence() {
    let model = ByteModel::new();
    let fixture =
        support::fixture_for_model(&model.binding()).expect("fixture");
    let data = b"# a\n\xff".to_vec();
    let state = native::view(&model.binding(), &fixture.state.stamp, &data);
    let intent = support::make_intent(&fixture, b"b").expect("intent");
    let receipt = receipt_faults::receipt(&fixture, &model, &state, &intent);
    let provider = Policies { fixture: &fixture, model: &model };
    let charge = fixture
        .representation
        .lowering()
        .bounds()
        .prepare(8, 16, 20, 1)
        .expect("lowering reservation");
    let request = RepresentationLoweringRequest::new(
        &fixture.representation,
        SemanticPolicyInput { intent: &intent, state: &state },
        &receipt,
        &fixture.buffer,
        charge,
        CancellationState::Open,
        DeadlineState::Pending,
    )
    .expect("validated receipt on exact state");
    let candidate = request.invoke(&provider).expect("selected provider");
    let rejected = candidate.finish(
        request,
        &fixture.representation,
        InstallationOutcome::Fenced(FenceCause::Cancellation),
    );
    assert_eq!(rejected.failure(), Some(SemanticPolicyFailure::Cancelled));
    let retained = rejected.plan().expect("lowered plan remains owned");
    assert_eq!(retained.operations().bytes(), [2, b'b']);
    assert_eq!(retained.preconditions().bytes(), b"a");
    assert_eq!(data, b"# a\n\xff");
}
