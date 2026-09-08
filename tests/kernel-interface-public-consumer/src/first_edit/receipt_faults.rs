use alloc::vec::Vec;

use reovim_kernel_interface::buffer::plan::{
    BufferPlanStamp, BufferTransactionIdentity,
};
use reovim_kernel_interface::cancellation::{
    CancellationState, DeadlineState, InstallationOutcome,
};
use reovim_kernel_interface::identity::IncarnationId;
use reovim_kernel_interface::lifecycle::{Generation, LifecycleIdentity};
use reovim_kernel_interface::resource::state::{
    ResourceStateModel, ResourceWorkingState,
};
use reovim_kernel_interface::semantic::descriptor::{
    RepresentationBehavior, RepresentationPolicyDescriptor,
    SemanticCoordinateSpaceDescriptor,
};
use reovim_kernel_interface::semantic::domain::{
    DomainValidationReceipt, DomainValidationRequest,
};
use reovim_kernel_interface::semantic::identity::ProjectionGeneration;
use reovim_kernel_interface::semantic::intent::{
    SemanticIntent, SemanticIntentBindingOutcome,
};
use reovim_kernel_interface::semantic::policy::{
    SemanticPolicyContractError, SemanticPolicyRequestError,
};
use reovim_kernel_interface::semantic::stamp::{
    CoordinateSpaceStamp, ProjectionStamp, SemanticCallerStamp,
    SemanticIntentStamp,
};
use reovim_kernel_interface::semantic::{
    representation::RepresentationLoweringRequest, state::SemanticPolicyInput,
};

use super::native::{models::ByteModel, support as native};
use super::policies::Policies;
use crate::semantic::support::{self, SemanticFixture};

#[derive(Clone, Copy)]
enum Mutation {
    Projection,
    Plan,
    Caller,
    Coordinate,
}

/// Same IDs never authorize reuse after another complete intent field changes.
///
/// # Panics
/// Panics when the public receipt/request boundary violates the witness.
pub fn exercise() {
    let model = ByteModel::new();
    let fixture =
        support::fixture_for_model(&model.binding()).expect("fixture");
    let data = b"# a\n\xff".to_vec();
    let observed = native::view(&model.binding(), &fixture.state.stamp, &data);
    let intent = support::make_intent(&fixture, b"b").expect("rename intent");
    let receipt = receipt(&fixture, &model, &observed, &intent);
    for mutation in [
        Mutation::Projection,
        Mutation::Plan,
        Mutation::Caller,
        Mutation::Coordinate,
    ] {
        let (descriptor, changed) = changed_intent(&fixture, &intent, mutation);
        assert!(!receipt.matches_intent(&fixture.domain, &changed));
        let charge = descriptor
            .lowering()
            .bounds()
            .prepare(8, 16, 20, 1)
            .expect("lowering reservation");
        let result = RepresentationLoweringRequest::new(
            &descriptor,
            SemanticPolicyInput { intent: &changed, state: &observed },
            &receipt,
            &fixture.buffer,
            charge,
            CancellationState::Open,
            DeadlineState::Pending,
        );
        assert_eq!(
            result.err(),
            Some(SemanticPolicyRequestError::Contract(
                SemanticPolicyContractError::ValidationReceiptMismatch
            ))
        );
        assert_eq!(data, b"# a\n\xff");
    }
}

pub fn receipt(
    fixture: &SemanticFixture,
    model: &ByteModel,
    observed: &ResourceWorkingState<'_, Vec<u8>>,
    intent: &SemanticIntent,
) -> DomainValidationReceipt {
    let provider = Policies { fixture, model };
    let charge = fixture
        .domain
        .validation()
        .bounds()
        .prepare(8, 16, 0, 1)
        .expect("Domain reservation");
    let request = DomainValidationRequest::new(
        &fixture.domain,
        &fixture.representation,
        SemanticPolicyInput { intent, state: observed },
        charge,
        CancellationState::Open,
        DeadlineState::Pending,
    )
    .expect("exact Domain observation");
    let candidate = request.invoke(&provider).expect("selected Domain");
    request
        .finish(&fixture.domain, candidate, InstallationOutcome::Current)
        .receipt()
        .expect("actual valid heading")
}

fn changed_intent(
    fixture: &SemanticFixture,
    intent: &SemanticIntent,
    mutation: Mutation,
) -> (RepresentationPolicyDescriptor, SemanticIntent) {
    let base = intent.stamp();
    let mut caller = base.caller();
    let mut plan = *base.plan();
    let mut projection = *base.projection();
    let mut coordinate = base.coordinate();
    match mutation {
        Mutation::Projection => {
            projection = ProjectionStamp::new(
                projection.projection(),
                projection.lifecycle(),
                projection.representation(),
                ProjectionGeneration::new(99),
            );
        }
        Mutation::Plan => {
            plan = BufferPlanStamp::new(
                plan.owner(),
                BufferTransactionIdentity::new(999),
                plan.history_transition(),
                plan.cancellation(),
                plan.correlation(),
                plan.deadline(),
            );
        }
        Mutation::Caller => {
            caller = SemanticCallerStamp::new(
                LifecycleIdentity::new(999, 998, Generation::new(1)),
                caller.role(),
            );
        }
        Mutation::Coordinate => {
            coordinate = CoordinateSpaceStamp::new(
                coordinate.space(),
                coordinate.contract(),
                IncarnationId::new(999),
            );
        }
    }
    let descriptor = with_coordinate(fixture, coordinate);
    let stamp = SemanticIntentStamp::new(
        base.intent(),
        caller,
        base.request(),
        *base.target(),
        plan,
        projection,
        coordinate,
    );
    let changed = bind_intent(&descriptor, stamp, intent.bytes());
    (descriptor, changed)
}

pub fn bind_intent(
    descriptor: &RepresentationPolicyDescriptor,
    stamp: SemanticIntentStamp,
    bytes: &[u8],
) -> SemanticIntent {
    let prepared = stamp
        .prepare_intent(
            descriptor,
            descriptor.intent().selected_stamp(),
            1,
            0,
            u64::try_from(bytes.len()).expect("bounded fixture intent"),
        )
        .expect("valid intent under selected Representation");
    let SemanticIntentBindingOutcome::Bound(intent) =
        prepared.bind(descriptor, bytes.into())
    else {
        panic!("intent bind")
    };
    intent
}

fn with_coordinate(
    fixture: &SemanticFixture,
    stamp: CoordinateSpaceStamp,
) -> RepresentationPolicyDescriptor {
    let old = fixture.representation.coordinate();
    let coordinate = SemanticCoordinateSpaceDescriptor::new(
        stamp,
        old.coordinate_schema(),
        old.region_schema(),
        old.value_schema(),
        old.intent_bounds(),
    );
    rebuild(fixture, &coordinate, fixture.representation.intent())
}

pub fn rebuild(
    fixture: &SemanticFixture,
    coordinate: &SemanticCoordinateSpaceDescriptor,
    intent: &reovim_kernel_interface::semantic::intent::SemanticIntentContract,
) -> RepresentationPolicyDescriptor {
    let old = &fixture.representation;
    let behavior = RepresentationBehavior::new(
        old.fidelity(),
        old.editability(),
        old.correspondence(),
    )
    .expect("existing behavior");
    RepresentationPolicyDescriptor::new(
        &fixture.domain,
        old.stamp(),
        coordinate,
        intent,
        behavior,
        old.plan(),
        old.lowering(),
    )
    .expect("coherent altered descriptor")
}
