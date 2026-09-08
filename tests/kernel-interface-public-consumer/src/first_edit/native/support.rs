use reovim_kernel_interface::bound::{
    CheckedByteCount, CheckedCount, ZeroPolicy,
};
use reovim_kernel_interface::buffer::carrier::{
    BufferCarrierRole, BufferPlanCarrier,
};
use reovim_kernel_interface::buffer::plan::{
    BufferCancellationIdentity, BufferCorrelationIdentity, BufferDeadline,
    BufferHistoryTransitionIdentity, BufferPlanBindingOutcome,
    BufferPlanDescriptor, BufferPlanStamp, BufferTransactionIdentity,
    BufferTransactionPlan,
};
use reovim_kernel_interface::buffer::{
    BufferHistoryEdge, BufferHistoryNodeIdentity, BufferHistoryStamp,
    BufferIdentity, BufferIncarnationStamp, BufferStateStamp, BufferVersion,
};
use reovim_kernel_interface::cancellation::{CancellationState, DeadlineState};
use reovim_kernel_interface::lifecycle::{Generation, LifecycleIdentity};
use reovim_kernel_interface::operation::ContractSelection;
use reovim_kernel_interface::resource::access::{
    ResourceAccessDescriptor, ResourceObservedState,
};
use reovim_kernel_interface::resource::operation::ResourceObservationTarget;
use reovim_kernel_interface::resource::state::{
    ResourceStateInput, ResourceStateLimits, ResourceStateModelBinding,
    ResourceStatePreparation, ResourceStateUsage, ResourceWorkingState,
};
use reovim_kernel_interface::resource::{
    ResourceAccessStamp, ResourceIdentity, ResourceRevision,
    ResourceSnapshotIdentity, ResourceSnapshotStamp, ResourceValueSchemas,
};

/// Reuses the selected Resource and Buffer fixture contracts.
///
/// # Panics
/// Panics if the shared fixture descriptors are incoherent.
#[must_use]
pub fn descriptors() -> (ResourceAccessDescriptor, BufferPlanDescriptor) {
    crate::resource_state_fixture()
        .expect("existing public descriptor fixtures")
}

/// Selects distinct native address/extent/unit schemas for a fixture model.
///
/// # Panics
/// The identity must leave room for its three adjacent fixture schema IDs.
#[must_use]
pub fn binding(identity: u128) -> ResourceStateModelBinding {
    let (resource, _) = descriptors();
    ResourceStateModelBinding {
        contract: selection(identity),
        lifecycle: LifecycleIdentity::new(identity, 1, Generation::new(1)),
        schemas: ResourceValueSchemas::new(
            selection(identity + 1),
            selection(identity + 2),
            selection(identity + 3),
            resource.schemas().revision(),
        ),
        payload: resource.carriers().payload().selected_stamp(),
    }
}

/// Selects version one of a fixture schema.
///
/// # Panics
/// Panics if the fixed compatible version window is incoherent.
#[must_use]
pub fn selection(identity: u128) -> ContractSelection {
    ContractSelection::new(identity, 1, 1, 1).expect("fixture schema")
}

/// Names native output/work reservations and bounded observation inputs.
#[must_use]
pub fn limits(resident: u64, work: u64) -> ResourceStateLimits {
    ResourceStateLimits {
        resident_bytes: bytes(resident),
        work_units: count(work),
        observation_count: count(4),
        observation_bytes: bytes(16),
    }
}

/// Reports fixture-model memory/work charges, not measured performance.
#[must_use]
pub fn usage(resident: u64, work: u64) -> ResourceStateUsage {
    ResourceStateUsage {
        resident_bytes: bytes(resident),
        work_units: count(work),
    }
}

/// Constructs a byte charge under the fixture's zero-allowing policy.
///
/// # Panics
/// Panics if that policy stops accepting the supplied quantity.
#[must_use]
pub fn bytes(value: u64) -> CheckedByteCount {
    CheckedByteCount::new(value, ZeroPolicy::Allow)
        .expect("named preparation byte reservation")
}

/// Constructs a model-work or observation reservation, allowing zero.
///
/// # Panics
/// Panics if that policy stops accepting the supplied quantity.
#[must_use]
pub fn count(value: u64) -> CheckedCount {
    CheckedCount::new(value, ZeroPolicy::Allow)
        .expect("named model work or observation reservation")
}

/// Binds actual data to the fixture's initial source snapshot.
#[must_use]
pub fn observation(bytes: &[u8]) -> ResourceObservedState {
    observation_at(snapshot(), bytes)
}

/// Binds actual data to a selected source observation.
///
/// # Panics
/// Panics on incompatible source schema or exceeded fixture payload bounds.
#[must_use]
pub fn observation_at(
    snapshot: ResourceSnapshotStamp,
    bytes: &[u8],
) -> ResourceObservedState {
    let (resource, _) = descriptors();
    let payload_stamp = resource.carriers().payload().selected_stamp();
    let prepared = resource
        .prepare_payload_carrier(
            payload_stamp,
            1,
            u64::try_from(bytes.len()).expect("fixture payload"),
        )
        .expect("bounded payload");
    let payload = resource
        .bind_payload_carrier(prepared, bytes.into())
        .expect("actual observed native data");
    ResourceObservedState::new(
        &resource,
        ResourceObservationTarget::Snapshot(snapshot),
        prepared.regions(),
        snapshot,
        payload,
    )
    .expect("coherent Resource observation")
}

/// Returns the fixture's initial native source identity and revision.
#[must_use]
pub fn snapshot() -> ResourceSnapshotStamp {
    let (resource, _) = descriptors();
    ResourceSnapshotStamp::new(
        ResourceAccessStamp::new(
            ResourceIdentity::new(41),
            LifecycleIdentity::new(42, 43, Generation::new(1)),
        ),
        ResourceRevision::new(resource.schemas().revision(), 7),
        ResourceSnapshotIdentity::new(44),
    )
}

/// Selects a Buffer version and opaque History node without tree arithmetic.
#[must_use]
pub fn state_stamp(version: u64, history: u128) -> BufferStateStamp {
    BufferStateStamp::new(
        snapshot(),
        BufferIncarnationStamp::new(
            BufferIdentity::new(51),
            LifecycleIdentity::new(52, 53, Generation::new(1)),
        ),
        BufferVersion::new(version),
        BufferHistoryStamp::new(
            LifecycleIdentity::new(54, 55, Generation::new(1)),
            BufferHistoryNodeIdentity::new(history),
        ),
    )
}

/// Names a Core-selected fixture tree edge; this does not create a tree.
///
/// # Panics
/// Panics if both endpoints identify the same node.
#[must_use]
pub fn edge(parent: u128, child: u128) -> BufferHistoryEdge {
    BufferHistoryEdge::new(
        BufferHistoryNodeIdentity::new(parent),
        BufferHistoryNodeIdentity::new(child),
    )
    .expect("fixture parent-child relation")
}

/// Binds a fixture transaction to its plan and causal authorities.
///
/// # Panics
/// The transaction ID must leave room for the fixture transition ID.
#[must_use]
pub fn plan_stamp(transaction: u128) -> BufferPlanStamp {
    let (_, descriptor) = descriptors();
    BufferPlanStamp::new(
        descriptor.owner(),
        BufferTransactionIdentity::new(transaction),
        BufferHistoryTransitionIdentity::new(transaction + 100),
        BufferCancellationIdentity::new(62),
        BufferCorrelationIdentity::new(63),
        BufferDeadline::None,
    )
}

/// Encodes native operations/preconditions under the default fixture request.
#[must_use]
pub fn plan(
    target: &BufferStateStamp,
    operations: &[u8],
    preconditions: &[u8],
) -> BufferTransactionPlan {
    plan_for(target, 61, operations, preconditions)
}

/// Encodes a complete native plan under an explicitly selected request.
///
/// # Panics
/// Panics when supplied carriers exceed the selected fixture's bounds.
#[must_use]
pub fn plan_for(
    target: &BufferStateStamp,
    transaction: u128,
    operations: &[u8],
    preconditions: &[u8],
) -> BufferTransactionPlan {
    let (_, descriptor) = descriptors();
    let outcome = descriptor.bind_plan(
        plan_stamp(transaction),
        *target,
        carrier(&descriptor, BufferCarrierRole::Origin, b"witness"),
        carrier(&descriptor, BufferCarrierRole::Operations, operations),
        carrier(&descriptor, BufferCarrierRole::Preconditions, preconditions),
    );
    let BufferPlanBindingOutcome::Accepted(plan) = outcome else {
        panic!("valid lowered fixture plan")
    };
    plan
}

fn carrier(
    descriptor: &BufferPlanDescriptor,
    role: BufferCarrierRole,
    bytes: &[u8],
) -> BufferPlanCarrier {
    let contract = match role {
        BufferCarrierRole::Origin => descriptor.carriers().origin(),
        BufferCarrierRole::Operations => descriptor.carriers().operations(),
        BufferCarrierRole::Preconditions => {
            descriptor.carriers().preconditions()
        }
    };
    descriptor
        .prepare_carrier(
            role,
            contract.selected_stamp(),
            1,
            u64::try_from(bytes.len()).expect("bounded native fixture"),
        )
        .expect("carrier shape")
        .bind(descriptor, bytes.into())
        .expect("native carrier data")
}

/// Borrows exact native data without copying it or granting mutation access.
#[must_use]
pub const fn view<'a, State>(
    model: &ResourceStateModelBinding,
    stamp: &BufferStateStamp,
    state: &'a State,
) -> ResourceWorkingState<'a, State> {
    ResourceWorkingState { model: *model, stamp: *stamp, state }
}

/// Validates fixture input against the selected native model/reservation.
///
/// # Panics
/// Panics if the supplied input is structurally mismatched or over budget.
#[must_use]
pub fn request<'a, State>(
    model: &ResourceStateModelBinding,
    input: ResourceStateInput<'a, State>,
) -> ResourceStatePreparation<'a, State> {
    ResourceStatePreparation::new(
        *model,
        input,
        limits(4096, 4096),
        CancellationState::Open,
        DeadlineState::Pending,
    )
    .expect("coherent state preparation")
}
