use reovim_kernel_interface::bound::{
    BoundDescriptor, BoundValue, CheckedByteCount, CheckedCount, ZeroPolicy,
};
use reovim_kernel_interface::buffer::BufferStateStamp;
use reovim_kernel_interface::carrier::CarrierSafetyDescriptor;
use reovim_kernel_interface::identity::InstanceId;
use reovim_kernel_interface::interaction::bounds::{
    InteractionIntentCharge, InteractionRequestBounds,
    InteractionSessionBounds, RepresentationResynchronizationBounds,
    RepresentationUpdateBounds, RepresentationUpdateFlowBounds,
    RepresentationUpdateShapeBounds,
};
use reovim_kernel_interface::interaction::descriptor::{
    InteractionCapabilitySupport, InteractionDescriptorError,
    InteractionSessionCapabilities, InteractionSessionDescriptor,
    InteractionSessionOperations, RepresentationAttachmentDescriptor,
    RepresentationAttachmentSelection,
};
use reovim_kernel_interface::interaction::identity::{
    ClientIdentity, InteractionCancellationIdentity,
    InteractionCorrelationIdentity, InteractionRequestIdentity,
    InteractionSessionIdentity, RepresentationAttachmentIdentity,
    RepresentationResynchronizationIdentity, RepresentationUpdateCursor,
    RepresentationUpdateIdentity, RepresentationUpdateStreamIdentity,
};
use reovim_kernel_interface::interaction::intent::{
    InteractionIntent, InteractionIntentBindingOutcome,
};
use reovim_kernel_interface::interaction::session::{
    InteractionClientStamp, InteractionDeadline, InteractionRequestStamp,
    InteractionResourceScope, InteractionSemanticBinding,
    InteractionSessionStamp, RepresentationAttachmentHandle,
    RepresentationUpdateStreamStamp,
};
use reovim_kernel_interface::interaction::update::{
    RepresentationUpdate, RepresentationUpdateBindingOutcome,
    RepresentationUpdateContract, RepresentationUpdateLineage,
    RepresentationUpdatePayloadClaim, RepresentationUpdateStamp,
};
use reovim_kernel_interface::lifecycle::{Generation, LifecycleIdentity};
use reovim_kernel_interface::semantic::identity::ProjectionGeneration;
use reovim_kernel_interface::semantic::intent::SemanticIntent;
use reovim_kernel_interface::semantic::stamp::ProjectionStamp;

use crate::semantic::support::SemanticFixture;

pub const UPDATE_BYTES: &[u8] = &[0x11, 0xa2, 0x00, 0xff];
pub const UPDATE_TYPE: u128 = 1_210;

pub struct InteractionFixture {
    pub semantic: SemanticFixture,
    pub session: InteractionSessionDescriptor,
    pub attachment: RepresentationAttachmentDescriptor,
}

pub fn fixture() -> Option<InteractionFixture> {
    fixture_for_semantic(crate::semantic::support::fixture()?)
}

pub fn fixture_for_semantic(
    semantic: SemanticFixture,
) -> Option<InteractionFixture> {
    let semantic_stamp =
        crate::semantic::support::intent_stamp(&semantic, 1_060, 1_061)?;
    let session_lifecycle = semantic_stamp.caller().authority();
    let client = InteractionClientStamp::new(
        ClientIdentity::new(1_200),
        LifecycleIdentity::new(1_201, 1_202, Generation::new(1)),
        semantic_stamp.caller().role(),
    );
    let session_stamp = InteractionSessionStamp::new(
        InteractionSessionIdentity::new(1_203),
        crate::resource::selection(1_204)?,
        session_lifecycle,
        client,
    );
    let scope = InteractionResourceScope::new(
        semantic_stamp.target().source().access(),
        semantic_stamp.target().buffer(),
    );
    let capabilities = all_capabilities()?;
    let bounds = session_bounds(session_lifecycle.instance())?;
    let update_contract = update_contract()?;
    let operations = InteractionSessionOperations::required();
    let session = InteractionSessionDescriptor::new(
        session_stamp,
        scope,
        capabilities,
        bounds,
        update_contract,
        operations,
    )
    .ok()?;
    let attachment_handle = RepresentationAttachmentHandle::new(
        RepresentationAttachmentIdentity::new(1_205),
        LifecycleIdentity::new(1_206, 1_207, Generation::new(1)),
        session_stamp.handle(),
    );
    let stream = RepresentationUpdateStreamStamp::new(
        RepresentationUpdateStreamIdentity::new(1_208),
        LifecycleIdentity::new(1_209, 1_210, Generation::new(1)),
        attachment_handle,
    );
    let selection = RepresentationAttachmentSelection::new(
        attachment_handle,
        scope,
        semantic.representation.stamp(),
        *semantic_stamp.projection(),
        semantic_stamp.coordinate(),
        stream,
        2,
    );
    let attachment = RepresentationAttachmentDescriptor::new(
        &session,
        &semantic.representation,
        selection,
    )
    .ok()?;
    Some(InteractionFixture { semantic, session, attachment })
}

pub fn replacement_session(
    fixture: &InteractionFixture,
) -> Option<InteractionSessionDescriptor> {
    let stamp = fixture.session.stamp();
    let lifecycle = stamp.lifecycle();
    let generation = lifecycle.generation().value().checked_add(1)?;
    let replacement = InteractionSessionStamp::new(
        stamp.session(),
        stamp.contract(),
        LifecycleIdentity::new(
            lifecycle.instance(),
            lifecycle.incarnation(),
            Generation::new(generation),
        ),
        stamp.client(),
    );
    InteractionSessionDescriptor::new(
        replacement,
        fixture.session.scope(),
        fixture.session.capabilities(),
        fixture.session.bounds().clone(),
        fixture.session.update_contract().clone(),
        fixture.session.operations(),
    )
    .ok()
}

pub fn foreign_intent_charge(bytes: u64) -> Option<InteractionIntentCharge> {
    session_bounds(9_999)?.requests().prepare_intent(bytes).ok()
}

pub fn replacement_attachment(
    fixture: &InteractionFixture,
) -> Option<RepresentationAttachmentDescriptor> {
    let current = fixture.attachment.handle();
    let life = current.lifecycle();
    let handle = RepresentationAttachmentHandle::new(
        current.attachment(),
        LifecycleIdentity::new(
            life.instance(),
            life.incarnation(),
            Generation::new(life.generation().value().checked_add(1)?),
        ),
        current.session(),
    );
    let current = fixture.attachment.stream();
    let life = current.lifecycle();
    let stream = RepresentationUpdateStreamStamp::new(
        current.stream(),
        LifecycleIdentity::new(
            life.instance(),
            life.incarnation(),
            Generation::new(life.generation().value().checked_add(1)?),
        ),
        handle,
    );
    RepresentationAttachmentDescriptor::new(
        &fixture.session,
        &fixture.semantic.representation,
        RepresentationAttachmentSelection::new(
            handle,
            fixture.attachment.scope(),
            fixture.attachment.representation(),
            fixture.attachment.projection(),
            fixture.attachment.coordinate(),
            stream,
            fixture.attachment.initial_credit().value(),
        ),
    )
    .ok()
}

pub fn session_without_resynchronization(
    fixture: &InteractionFixture,
) -> Option<InteractionSessionDescriptor> {
    let supported = InteractionCapabilitySupport::Supported;
    let capabilities = InteractionSessionCapabilities::new(
        supported,
        supported,
        supported,
        supported,
        InteractionCapabilitySupport::Unsupported,
    )
    .ok()?;
    session_with_capabilities(fixture, capabilities)
}

pub fn session_without_deltas(
    fixture: &InteractionFixture,
) -> Option<InteractionSessionDescriptor> {
    let supported = InteractionCapabilitySupport::Supported;
    let capabilities = InteractionSessionCapabilities::new(
        supported,
        supported,
        supported,
        InteractionCapabilitySupport::Unsupported,
        supported,
    )
    .ok()?;
    session_with_capabilities(fixture, capabilities)
}

fn session_with_capabilities(
    fixture: &InteractionFixture,
    capabilities: InteractionSessionCapabilities,
) -> Option<InteractionSessionDescriptor> {
    InteractionSessionDescriptor::new(
        fixture.session.stamp(),
        fixture.session.scope(),
        capabilities,
        fixture.session.bounds().clone(),
        fixture.session.update_contract().clone(),
        fixture.session.operations(),
    )
    .ok()
}

pub fn descriptor_cases() -> bool {
    let Some(fixture) = fixture() else { return false };
    let capabilities = fixture.session.capabilities();
    let bounds = fixture.session.bounds();
    fixture.session.stamp().session().value() == 1_203
        && fixture.session.scope().same_scope(fixture.attachment.scope())
        && capabilities.cancellation()
            == InteractionCapabilitySupport::Supported
        && capabilities.reconciliation()
            == InteractionCapabilitySupport::Supported
        && capabilities.updates() == InteractionCapabilitySupport::Supported
        && capabilities.deltas() == InteractionCapabilitySupport::Supported
        && capabilities.resynchronization()
            == InteractionCapabilitySupport::Supported
        && bounds.charge_owner().value()
            == fixture.session.stamp().lifecycle().instance()
        && fixture.attachment.initial_credit().value() == 2
        && fixture.session.update_contract().schema().identity() == UPDATE_TYPE
        && descriptor_owner_fault(&fixture)
}

fn descriptor_owner_fault(fixture: &InteractionFixture) -> bool {
    let stamp = fixture.session.stamp();
    let lifecycle = stamp.lifecycle();
    let Some(instance) = lifecycle.instance().checked_add(1) else {
        return false;
    };
    let mismatched = InteractionSessionStamp::new(
        stamp.session(),
        stamp.contract(),
        LifecycleIdentity::new(
            instance,
            lifecycle.incarnation(),
            lifecycle.generation(),
        ),
        stamp.client(),
    );
    let result = InteractionSessionDescriptor::new(
        mismatched,
        fixture.session.scope(),
        fixture.session.capabilities(),
        fixture.session.bounds().clone(),
        fixture.session.update_contract().clone(),
        fixture.session.operations(),
    );
    match result {
        Err(InteractionDescriptorError::ChargeOwnerMismatch {
            expected,
            actual,
        }) => expected.value() != actual.value(),
        _ => false,
    }
}

pub fn make_semantic_intent(
    fixture: &InteractionFixture,
    intent: u128,
    request: u128,
) -> Option<SemanticIntent> {
    crate::semantic::support::make_intent_with(
        &fixture.semantic,
        crate::semantic::support::INTENT_BYTES,
        intent,
        request,
    )
}

pub fn request_stamp(
    fixture: &InteractionFixture,
    semantic: &SemanticIntent,
    request: u128,
) -> Option<InteractionRequestStamp> {
    let stamp = semantic.stamp();
    let plan = stamp.plan();
    let binding = InteractionSemanticBinding::new(
        stamp.request(),
        stamp.intent(),
        plan.cancellation(),
        plan.correlation(),
        plan.deadline(),
        plan.transaction(),
        plan.history_transition(),
    );
    InteractionRequestStamp::new(
        fixture.session.stamp().handle(),
        fixture.attachment.handle(),
        InteractionRequestIdentity::new(request),
        InteractionCancellationIdentity::new(request + 1),
        InteractionCorrelationIdentity::new(request + 2),
        InteractionDeadline::None,
        binding,
    )
    .ok()
}

pub fn make_intent(
    fixture: &InteractionFixture,
    interaction_request: u128,
    semantic_intent: u128,
    semantic_request: u128,
) -> Option<InteractionIntent> {
    let semantic =
        make_semantic_intent(fixture, semantic_intent, semantic_request)?;
    let stamp = request_stamp(fixture, &semantic, interaction_request)?;
    let charge = fixture
        .session
        .bounds()
        .requests()
        .prepare_intent(semantic.shape().bytes().value())
        .ok()?;
    match InteractionIntent::bind(
        &fixture.session,
        &fixture.attachment,
        stamp,
        charge,
        semantic,
        reovim_kernel_interface::cancellation::CancellationState::Open,
        reovim_kernel_interface::cancellation::DeadlineState::Pending,
    ) {
        InteractionIntentBindingOutcome::Bound(intent) => Some(intent),
        InteractionIntentBindingOutcome::Rejected(_) => None,
    }
}

pub fn initial_update(
    fixture: &InteractionFixture,
) -> Option<RepresentationUpdate> {
    let target = crate::buffer::state(1, 0, 0)?;
    let projection = projection(fixture, 3);
    let stamp = update_stamp(
        fixture,
        1_300,
        RepresentationUpdateCursor::INITIAL,
        &target,
        &projection,
    )?;
    let lineage = RepresentationUpdateLineage::FullSnapshot {
        supersedes: ProjectionGeneration::new(3),
        result: ProjectionGeneration::new(3),
    };
    payload_update(fixture, stamp, lineage, UPDATE_BYTES)
}

pub fn delta_update(
    fixture: &InteractionFixture,
) -> Option<RepresentationUpdate> {
    let target = crate::buffer::state(1, 1, 1)?;
    let stamp = update_stamp(
        fixture,
        1_301,
        RepresentationUpdateCursor::new(1),
        &target,
        &projection(fixture, 3),
    )?;
    let lineage = RepresentationUpdateLineage::Delta {
        predecessor: ProjectionGeneration::new(3),
        result: ProjectionGeneration::new(4),
    };
    payload_update(fixture, stamp, lineage, UPDATE_BYTES)
}

pub fn gap_update(
    fixture: &InteractionFixture,
) -> Option<RepresentationUpdate> {
    let target = crate::buffer::state(1, 1, 1)?;
    let stamp = update_stamp(
        fixture,
        1_302,
        RepresentationUpdateCursor::new(2),
        &target,
        &projection(fixture, 4),
    )?;
    RepresentationUpdate::gap(
        &fixture.session,
        &fixture.attachment,
        stamp,
        ProjectionGeneration::new(4),
        ProjectionGeneration::new(6),
        reovim_kernel_interface::interaction::update::
            RepresentationGapCause::RetentionEvicted,
        RepresentationResynchronizationIdentity::new(1_303),
    )
    .ok()
}

pub fn resync_update(
    fixture: &InteractionFixture,
) -> Option<RepresentationUpdate> {
    let target = crate::buffer::state(1, 1, 1)?;
    let stamp = update_stamp(
        fixture,
        1_304,
        RepresentationUpdateCursor::new(3),
        &target,
        &projection(fixture, 4),
    )?;
    let lineage = RepresentationUpdateLineage::Resynchronization {
        result: ProjectionGeneration::new(6),
        resynchronization: RepresentationResynchronizationIdentity::new(1_303),
    };
    payload_update(fixture, stamp, lineage, UPDATE_BYTES)
}

pub fn update_stamp(
    fixture: &InteractionFixture,
    update: u128,
    cursor: RepresentationUpdateCursor,
    target: &BufferStateStamp,
    projection: &ProjectionStamp,
) -> Option<RepresentationUpdateStamp> {
    RepresentationUpdateStamp::new(
        &fixture.session,
        &fixture.attachment,
        RepresentationUpdateIdentity::new(update),
        cursor,
        target,
        projection,
        &fixture.attachment.coordinate(),
    )
    .ok()
}

pub const fn projection(
    fixture: &InteractionFixture,
    predecessor: u64,
) -> ProjectionStamp {
    let attached = fixture.attachment.projection();
    ProjectionStamp::new(
        attached.projection(),
        attached.lifecycle(),
        attached.representation(),
        ProjectionGeneration::new(predecessor),
    )
}

pub fn payload_update(
    fixture: &InteractionFixture,
    stamp: RepresentationUpdateStamp,
    lineage: RepresentationUpdateLineage,
    bytes: &[u8],
) -> Option<RepresentationUpdate> {
    let size = u64::try_from(bytes.len()).ok()?;
    let preparation = fixture
        .session
        .prepare_update(
            &fixture.attachment,
            stamp,
            lineage,
            RepresentationUpdatePayloadClaim::new(
                fixture.session.update_contract().selected_stamp(),
                1,
                0,
                size,
            ),
        )
        .ok()?;
    match preparation.bind(bytes.into()) {
        RepresentationUpdateBindingOutcome::Bound(carrier) => {
            Some(carrier.into_update())
        }
        RepresentationUpdateBindingOutcome::Rejected(_) => None,
    }
}

fn all_capabilities() -> Option<InteractionSessionCapabilities> {
    InteractionSessionCapabilities::new(
        InteractionCapabilitySupport::Supported,
        InteractionCapabilitySupport::Supported,
        InteractionCapabilitySupport::Supported,
        InteractionCapabilitySupport::Supported,
        InteractionCapabilitySupport::Supported,
    )
    .ok()
}

fn session_bounds(owner: u128) -> Option<InteractionSessionBounds> {
    bounds_with_reservations(owner, 256, 8, 64)
}

pub fn bounds_with_reservations(
    owner: u128,
    outcome_bytes: u64,
    resynchronization_work: u64,
    resynchronization_bytes: u64,
) -> Option<InteractionSessionBounds> {
    let requests = InteractionRequestBounds::new(
        InstanceId::new(owner),
        count_bound(owner, 1, 4, ZeroPolicy::Reject)?,
        byte_bound(owner, 64, 256, ZeroPolicy::Reject)?,
        count_bound(owner, 1, 4, ZeroPolicy::Reject)?,
        byte_bound(owner, outcome_bytes, 1_024, ZeroPolicy::Reject)?,
    )
    .ok()?;
    let shape = RepresentationUpdateShapeBounds::new(
        count_bound(owner, 8, 8, ZeroPolicy::Reject)?,
        count_bound(owner, 4, 4, ZeroPolicy::Allow)?,
        byte_bound(owner, 64, 512, ZeroPolicy::Reject)?,
    );
    let resynchronization = RepresentationResynchronizationBounds::new(
        count_bound(owner, 1, 2, ZeroPolicy::Reject)?,
        count_bound(owner, resynchronization_work, 16, ZeroPolicy::Reject)?,
        byte_bound(owner, resynchronization_bytes, 512, ZeroPolicy::Reject)?,
    );
    let flow = RepresentationUpdateFlowBounds::new(
        count_bound(owner, 1, 8, ZeroPolicy::Reject)?,
        byte_bound(owner, 64, 512, ZeroPolicy::Allow)?,
        count_bound(owner, 4, 4, ZeroPolicy::Reject)?,
        count_bound(owner, 1, 1, ZeroPolicy::Reject)?,
        resynchronization,
    );
    let updates =
        RepresentationUpdateBounds::new(InstanceId::new(owner), shape, flow)
            .ok()?;
    InteractionSessionBounds::new(
        InstanceId::new(owner),
        requests,
        updates,
        count_bound(owner, 16, 16, ZeroPolicy::Reject)?,
    )
    .ok()
}

fn update_contract() -> Option<RepresentationUpdateContract> {
    RepresentationUpdateContract::new(
        crate::resource::selection(UPDATE_TYPE)?,
        update_safety()?,
    )
    .ok()
}

fn update_safety() -> Option<CarrierSafetyDescriptor> {
    crate::semantic::support::carrier_safety(UPDATE_TYPE)
}

fn count_bound(
    owner: u128,
    per_object: u64,
    aggregate: u64,
    zero: ZeroPolicy,
) -> Option<BoundDescriptor> {
    BoundDescriptor::new(
        BoundValue::Count(CheckedCount::new(per_object, zero).ok()?),
        BoundValue::Count(CheckedCount::new(aggregate, zero).ok()?),
        zero,
        InstanceId::new(owner),
    )
    .ok()
}

fn byte_bound(
    owner: u128,
    per_object: u64,
    aggregate: u64,
    zero: ZeroPolicy,
) -> Option<BoundDescriptor> {
    BoundDescriptor::new(
        BoundValue::Bytes(CheckedByteCount::new(per_object, zero).ok()?),
        BoundValue::Bytes(CheckedByteCount::new(aggregate, zero).ok()?),
        zero,
        InstanceId::new(owner),
    )
    .ok()
}
