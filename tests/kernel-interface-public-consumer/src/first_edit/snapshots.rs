use alloc::boxed::Box;

use reovim_kernel_interface::buffer::BufferStateStamp;
use reovim_kernel_interface::cancellation::{
    CancellationState, DeadlineState, InstallationOutcome,
};
use reovim_kernel_interface::interaction::identity::RepresentationUpdateCursor;
use reovim_kernel_interface::interaction::update::{
    RepresentationUpdate, RepresentationUpdateBindingOutcome,
    RepresentationUpdateLineage, RepresentationUpdatePayloadClaim,
};
use reovim_kernel_interface::resource::state::ResourceWorkingState;
use reovim_kernel_interface::semantic::descriptor::{
    SemanticPolicyCallDescriptor, SemanticPolicyConcurrency,
};
use reovim_kernel_interface::semantic::identity::ProjectionGeneration;
use reovim_kernel_interface::semantic::policy::SemanticPolicyFailure;
use reovim_kernel_interface::semantic::snapshot::{
    RepresentationSnapshotDescriptor, RepresentationSnapshotLimits,
    RepresentationSnapshotPayload, RepresentationSnapshotPort,
    RepresentationSnapshotRequest, RepresentationSnapshotSchema,
};
use reovim_kernel_interface::semantic::stamp::ProjectionStamp;

use super::families::Family;
use super::native::support as native;
use crate::interaction::support as interaction;
use crate::semantic::support::{self, SemanticFixture};

pub struct SnapshotPolicy<'a, Model> {
    pub descriptor: RepresentationSnapshotDescriptor,
    pub model: &'a Model,
}

impl<Model: Family> RepresentationSnapshotPort for SnapshotPolicy<'_, Model> {
    type State = Model::State;

    fn descriptor(&self) -> &RepresentationSnapshotDescriptor {
        &self.descriptor
    }

    fn snapshot(
        &self,
        request: &RepresentationSnapshotRequest<'_, Self::State>,
    ) -> Result<RepresentationSnapshotPayload, SemanticPolicyFailure> {
        Ok(RepresentationSnapshotPayload {
            schema: request.descriptor().output_schema(),
            // One complete semantic snapshot object, even for an empty map.
            items: native::count(1),
            depth: native::count(0),
            bytes: self.model.preview(request.observed().state)?,
        })
    }
}

pub fn descriptor(
    fixture: &SemanticFixture,
) -> RepresentationSnapshotDescriptor {
    let call = SemanticPolicyCallDescriptor::new(
        SemanticPolicyConcurrency::Serialized,
        &support::call_bounds(1800, 16).expect("snapshot call reservation"),
    )
    .expect("snapshot serialized policy");
    let interaction = delivery_fixture(fixture);
    RepresentationSnapshotDescriptor::new(
        &fixture.representation,
        interaction.session.update_contract().schema(),
        &call,
        RepresentationSnapshotLimits {
            items: native::count(1),
            depth: native::count(0),
        },
    )
}

pub fn produce<Model: Family>(
    model: &Model,
    fixture: &SemanticFixture,
    observed: &ResourceWorkingState<'_, Model::State>,
    projection: &ProjectionStamp,
    result: ProjectionGeneration,
    cursor: RepresentationUpdateCursor,
) -> (Box<[u8]>, RepresentationUpdate) {
    let provider = SnapshotPolicy { descriptor: descriptor(fixture), model };
    let charge = provider
        .descriptor
        .call()
        .bounds()
        .prepare(8, 16, 16, 1)
        .expect("bounded snapshot work and output");
    let request = RepresentationSnapshotRequest::new(
        &provider.descriptor,
        observed,
        *projection,
        charge,
        CancellationState::Open,
        DeadlineState::Pending,
    )
    .expect("actual state snapshot request");
    let snapshot = request.produce_with(&provider).expect("derived snapshot");
    snapshot
        .check_admission(
            &provider.descriptor,
            &observed.stamp,
            projection,
            InstallationOutcome::Current,
        )
        .expect("exact state and Projection retained after interpretation");
    bind_update(
        fixture,
        &observed.stamp,
        projection,
        result,
        cursor,
        snapshot.into_payload(),
    )
}

fn bind_update(
    fixture: &SemanticFixture,
    target: &BufferStateStamp,
    projection: &ProjectionStamp,
    result: ProjectionGeneration,
    cursor: RepresentationUpdateCursor,
    payload: RepresentationSnapshotPayload,
) -> (Box<[u8]>, RepresentationUpdate) {
    let interaction = delivery_fixture(fixture);
    let selected = interaction.session.update_contract().selected_stamp();
    assert_eq!(
        payload.schema,
        RepresentationSnapshotSchema::new(
            selected.type_identity(),
            selected.schema_version(),
        )
    );
    // Projection/Interaction assigns delivery; the snapshot provider cannot.
    let stamp = interaction::update_stamp(
        &interaction,
        1900 + u128::from(cursor.value()),
        cursor,
        target,
        projection,
    )
    .expect("Projection-owned delivery identity and target");
    let lineage = RepresentationUpdateLineage::FullSnapshot {
        supersedes: projection.predecessor(),
        result,
    };
    let bytes = payload.bytes.clone();
    let claim = RepresentationUpdatePayloadClaim::new(
        selected,
        payload.items.value(),
        payload.depth.value(),
        u64::try_from(payload.bytes.len()).expect("bounded snapshot bytes"),
    );
    let preparation = interaction
        .session
        .prepare_update(&interaction.attachment, stamp, lineage, claim)
        .expect("same semantic schema, shape and data retained for delivery");
    let RepresentationUpdateBindingOutcome::Bound(carrier) =
        preparation.bind(payload.bytes)
    else {
        panic!("snapshot update bind")
    };
    let update = carrier.into_update();
    update
        .validate(&interaction.session, &interaction.attachment)
        .expect("public consumer accepts exact delivery binding");
    (bytes, update)
}

fn delivery_fixture(
    fixture: &SemanticFixture,
) -> interaction::InteractionFixture {
    let semantic = support::fixture_for_model(fixture.domain.state_model())
        .expect("same selected semantic contracts");
    interaction::fixture_for_semantic(semantic)
        .expect("existing Interaction family")
}
