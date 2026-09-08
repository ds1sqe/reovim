use reovim_kernel_core::{Buffer, BufferConfig, BufferLimits};
use reovim_kernel_interface::buffer::carrier::BufferCarrierRole;
use reovim_kernel_interface::buffer::plan::{
    BufferCancellationIdentity, BufferCorrelationIdentity, BufferDeadline,
    BufferHistoryTransitionIdentity, BufferPlanBindingOutcome, BufferPlanStamp,
    BufferTransactionIdentity, BufferTransactionPlan,
};
use reovim_kernel_interface::buffer::{BufferIdentity, BufferIncarnationStamp};
use reovim_kernel_interface::cancellation::{
    CancellationState, DeadlineState, InstallationOutcome,
};
use reovim_kernel_interface::lifecycle::{Generation, LifecycleIdentity};
use reovim_kernel_interface::resource::access::ResourceObservedState;
use reovim_kernel_interface::resource::delivery::ResourceAdmissionStatus;
use reovim_kernel_interface::resource::port::ResourceAccessPort;
use reovim_kernel_interface::resource::state::{
    ResourcePreparationStamp, ResourcePreparedState, ResourceStateInput,
    ResourceStateLimits, ResourceStateModel, ResourceStatePreparation,
};
use reovim_kernel_interface::resource::{
    ResourceAccessStamp, ResourceIdentity,
};
use reovim_resource_local::{ByteModel, ByteState, MemoryResource, profile};

pub fn opened<Model: ResourceStateModel>(
    model: &Model,
    source: &[u8],
    nodes: u64,
    resident: u64,
) -> (MemoryResource, Buffer<Model::State>) {
    let (source, observed) = observation(model, source);
    let request = preparation(model, ResourceStateInput::Initial(&observed));
    let candidate = request.prepare_with(model).unwrap();
    let buffer = Buffer::new(
        &config(model, &observed, nodes, resident),
        &request.stamp(),
        candidate,
        InstallationOutcome::Current,
    )
    .unwrap();
    (source, buffer)
}

pub fn config(
    model: &impl ResourceStateModel,
    observed: &ResourceObservedState,
    nodes: u64,
    resident: u64,
) -> BufferConfig {
    BufferConfig {
        source: observed.snapshot(),
        incarnation: BufferIncarnationStamp::new(
            BufferIdentity::new(3),
            life(4),
        ),
        history: life(5),
        model: model.binding(),
        plan_owner: model.plan_descriptor().owner(),
        limits: BufferLimits {
            history_nodes: profile::count(nodes),
            resident_bytes: profile::bytes(resident),
        },
        preparation: limits(),
    }
}

pub fn prepared(
    model: &ByteModel,
    buffer: &Buffer<ByteState>,
    id: u128,
    replacement: u8,
) -> (ResourcePreparationStamp, ResourcePreparedState<ByteState>) {
    let lease = buffer.observe();
    let view = lease.view();
    let mut operations = 2_u64.to_le_bytes().to_vec();
    operations.push(replacement);
    let plan =
        plan(model, &view.stamp, id, &operations, &[view.state.as_bytes()[2]]);
    let request = preparation(
        model,
        ResourceStateInput::Edit {
            observed: &view,
            descriptor: model.plan_descriptor(),
            plan: &plan,
            observations: &[],
        },
    );
    (request.stamp(), request.prepare_with(model).unwrap())
}

pub fn plan(
    model: &impl ResourceStateModel,
    target: &reovim_kernel_interface::buffer::BufferStateStamp,
    id: u128,
    operations: &[u8],
    preconditions: &[u8],
) -> BufferTransactionPlan {
    let descriptor = model.plan_descriptor();
    let stamp = stamp(model, id);
    let bind = |role, schema, bytes: &[u8]| {
        descriptor
            .prepare_carrier(role, schema, 1, bytes.len() as u64)
            .unwrap()
            .bind(descriptor, bytes.into())
            .unwrap()
    };
    let origin = bind(
        BufferCarrierRole::Origin,
        descriptor.carriers().origin().selected_stamp(),
        &[1],
    );
    let operations = bind(
        BufferCarrierRole::Operations,
        descriptor.carriers().operations().selected_stamp(),
        operations,
    );
    let preconditions = bind(
        BufferCarrierRole::Preconditions,
        descriptor.carriers().preconditions().selected_stamp(),
        preconditions,
    );
    let BufferPlanBindingOutcome::Accepted(plan) =
        descriptor.bind_plan(stamp, *target, origin, operations, preconditions)
    else {
        panic!("selected byte-native operation")
    };
    plan
}

pub fn observation(
    model: &impl ResourceStateModel,
    source: &[u8],
) -> (MemoryResource, ResourceObservedState) {
    let mut resource = MemoryResource::new(
        &model.binding(),
        ResourceAccessStamp::new(ResourceIdentity::new(6), life(7)),
        source.to_vec(),
    )
    .unwrap();
    let open = resource.open_request(1);
    assert_eq!(
        resource.submit_open(open).status(),
        ResourceAdmissionStatus::Admitted
    );
    resource.take_open_completion(open.operation()).into_completion().unwrap();
    let observation = resource.observation_request(2);
    let operation = observation.operation();
    assert_eq!(
        resource.submit_observation(observation).status(),
        ResourceAdmissionStatus::Admitted
    );
    let (_, outcome) = resource
        .take_observation_completion(operation)
        .into_completion()
        .unwrap()
        .into_parts();
    (resource, outcome.into_observed_state().unwrap())
}

pub fn preparation<'a, Model: ResourceStateModel>(
    model: &Model,
    input: ResourceStateInput<'a, Model::State>,
) -> ResourceStatePreparation<'a, Model::State> {
    ResourceStatePreparation::new(
        model.binding(),
        input,
        limits(),
        CancellationState::Open,
        DeadlineState::Pending,
    )
    .unwrap()
}

pub fn stamp(model: &impl ResourceStateModel, id: u128) -> BufferPlanStamp {
    BufferPlanStamp::new(
        model.plan_descriptor().owner(),
        BufferTransactionIdentity::new(id),
        BufferHistoryTransitionIdentity::new(id),
        BufferCancellationIdentity::new(id),
        BufferCorrelationIdentity::new(id),
        BufferDeadline::None,
    )
}

pub fn limits() -> ResourceStateLimits {
    ResourceStateLimits {
        resident_bytes: profile::bytes(4096),
        work_units: profile::count(4096),
        observation_count: profile::count(1),
        observation_bytes: profile::bytes(4096),
    }
}

pub const fn life(id: u128) -> LifecycleIdentity {
    LifecycleIdentity::new(id, 1, Generation::new(1))
}
