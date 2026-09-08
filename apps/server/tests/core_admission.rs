use reovim_kernel_core::{Buffer, BufferConfig, BufferError, BufferLimits};
use reovim_kernel_interface::buffer::carrier::BufferCarrierRole;
use reovim_kernel_interface::buffer::plan::{
    BufferCancellationIdentity, BufferCorrelationIdentity, BufferDeadline,
    BufferHistoryTransitionIdentity, BufferPlanBindingOutcome, BufferPlanStamp,
    BufferTransactionIdentity, BufferTransactionPlan,
};
use reovim_kernel_interface::buffer::{BufferIdentity, BufferIncarnationStamp};
use reovim_kernel_interface::cancellation::{
    CancellationState, DeadlineState, FenceCause, InstallationOutcome,
};
use reovim_kernel_interface::lifecycle::{Generation, LifecycleIdentity};
use reovim_kernel_interface::resource::access::ResourceObservedState;
use reovim_kernel_interface::resource::delivery::ResourceAdmissionStatus;
use reovim_kernel_interface::resource::port::ResourceAccessPort;
use reovim_kernel_interface::resource::state::{
    ResourcePreparationStamp, ResourcePreparedState, ResourceStateError,
    ResourceStateInput, ResourceStateLimits, ResourceStateModel,
    ResourceStatePreparation,
};
use reovim_kernel_interface::resource::{
    ResourceAccessStamp, ResourceIdentity,
};
use reovim_resource_local::{ByteModel, ByteState, MemoryResource, profile};

#[test]
fn repeated_acceptance_owns_versions_and_retains_immutable_native_leases() {
    let model = ByteModel::new(life(1), life(2));
    let (_source, mut buffer) = opened(&model, 8, 4096);
    let initial = buffer.observe();
    let cloned_handle = initial.clone(); // ByteState deliberately is not Clone.
    for (request, byte) in [(10, b'b'), (11, b'c'), (12, b'd')] {
        let prior = buffer.stamp();
        let (expected, candidate) = prepared(&model, &buffer, request, byte);
        let staged = buffer.stage_edit(&expected, candidate).unwrap();
        let commit =
            buffer.commit_edit(staged, InstallationOutcome::Current).unwrap();
        assert_eq!(commit.prior, prior);
        assert_eq!(
            commit.result.version().value(),
            prior.version().value() + 1
        );
        assert_eq!(commit.edge.parent(), prior.history().node());
        assert_eq!(commit.edge.child(), commit.result.history().node());
        assert_ne!(commit.edge.parent(), commit.edge.child());
        assert_eq!(
            buffer.observe().view().state.as_bytes(),
            [b'#', b' ', byte, b'\n', 0xff]
        );
    }
    assert_eq!(initial.view().state.as_bytes(), b"# a\n\xff");
    assert_eq!(cloned_handle.view().state.as_bytes(), b"# a\n\xff");
    assert_eq!(initial.stamp(), cloned_handle.stamp());
}

#[test]
fn stale_and_fenced_candidates_never_change_accepted_state() {
    let model = ByteModel::new(life(1), life(2));
    let (_source, mut buffer) = opened(&model, 8, 4096);
    let (first, a) = prepared(&model, &buffer, 10, b'b');
    let (second, b) = prepared(&model, &buffer, 11, b'c');
    let a = buffer.stage_edit(&first, a).unwrap();
    let b = buffer.stage_edit(&second, b).unwrap();
    buffer.commit_edit(a, InstallationOutcome::Current).unwrap();
    let accepted = buffer.stamp();
    let rejected =
        buffer.commit_edit(b, InstallationOutcome::Current).unwrap_err();
    assert_eq!(
        rejected.cause,
        BufferError::State(ResourceStateError::StateMismatch)
    );
    drop(rejected); // Returned ownership leaves the authority call first.
    let (expected, candidate) = prepared(&model, &buffer, 12, b'd');
    let staged = buffer.stage_edit(&expected, candidate).unwrap();
    let fenced = buffer
        .commit_edit(
            staged,
            InstallationOutcome::Fenced(FenceCause::Cancellation),
        )
        .unwrap_err();
    assert_eq!(
        fenced.cause,
        BufferError::State(ResourceStateError::Fenced(
            FenceCause::Cancellation
        ))
    );
    drop(fenced);
    assert_eq!(buffer.stamp(), accepted);
    assert_eq!(buffer.observe().view().state.as_bytes(), b"# b\n\xff");
}

#[test]
fn node_and_native_byte_pressure_return_the_unchanged_candidate() {
    let model = ByteModel::new(life(1), life(2));
    let (_initial_source, initial) = observation(&model);
    let request = preparation(&model, ResourceStateInput::Initial(&initial));
    let native_charge =
        request.prepare_with(&model).unwrap().usage().resident_bytes;
    for (nodes, resident) in [(1, 4096), (8, native_charge.value())] {
        let (_source, buffer) = opened(&model, nodes, resident);
        let initial_stamp = buffer.stamp();
        let (expected, candidate) = prepared(&model, &buffer, 10, b'b');
        let rejected = buffer.stage_edit(&expected, candidate).unwrap_err();
        assert_eq!(rejected.cause, BufferError::Capacity);
        assert_eq!(rejected.value.state().as_bytes(), b"# b\n\xff");
        assert_eq!(buffer.stamp(), initial_stamp);
        assert_eq!(buffer.observe().view().state.as_bytes(), b"# a\n\xff");
    }
}

#[test]
fn changed_preparation_and_initial_fences_retain_native_ownership() {
    let model = ByteModel::new(life(1), life(2));
    let (_source, buffer) = opened(&model, 8, 4096);
    let (expected, candidate) = prepared(&model, &buffer, 10, b'b');
    let (different, _) = prepared(&model, &buffer, 11, b'c');
    assert_ne!(expected, different);
    let rejected = buffer.stage_edit(&different, candidate).unwrap_err();
    assert_eq!(
        rejected.cause,
        BufferError::State(ResourceStateError::PreparationMismatch)
    );
    assert_eq!(rejected.value.state().as_bytes(), b"# b\n\xff");
    let (_other_source, observed) = observation(&model);
    let request = preparation(&model, ResourceStateInput::Initial(&observed));
    let candidate = request.prepare_with(&model).unwrap();
    let config = config(&model, &observed, 8, 4096);
    let rejected = Buffer::new(
        &config,
        &request.stamp(),
        candidate,
        InstallationOutcome::Fenced(FenceCause::Closing),
    )
    .unwrap_err();
    assert_eq!(
        rejected.cause,
        BufferError::State(ResourceStateError::Fenced(FenceCause::Closing))
    );
    assert_eq!(rejected.value.state().as_bytes(), b"# a\n\xff");
}

fn opened(
    model: &ByteModel,
    nodes: u64,
    resident: u64,
) -> (MemoryResource, Buffer<ByteState>) {
    let (source, observed) = observation(model);
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

fn config(
    model: &ByteModel,
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
        plan_owner: life(2),
        limits: BufferLimits {
            history_nodes: profile::count(nodes),
            resident_bytes: profile::bytes(resident),
        },
        preparation: limits(),
    }
}

fn prepared(
    model: &ByteModel,
    buffer: &Buffer<ByteState>,
    id: u128,
    replacement: u8,
) -> (ResourcePreparationStamp, ResourcePreparedState<ByteState>) {
    let lease = buffer.observe();
    let view = lease.view();
    let plan =
        plan(model, &view.stamp, id, view.state.as_bytes()[2], replacement);
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

fn plan(
    model: &ByteModel,
    target: &reovim_kernel_interface::buffer::BufferStateStamp,
    id: u128,
    expected: u8,
    replacement: u8,
) -> BufferTransactionPlan {
    let descriptor = model.plan_descriptor();
    let stamp = BufferPlanStamp::new(
        descriptor.owner(),
        BufferTransactionIdentity::new(id),
        BufferHistoryTransitionIdentity::new(id),
        BufferCancellationIdentity::new(id),
        BufferCorrelationIdentity::new(id),
        BufferDeadline::None,
    );
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
    let mut operations = 2_u64.to_le_bytes().to_vec();
    operations.push(replacement);
    let operations = bind(
        BufferCarrierRole::Operations,
        descriptor.carriers().operations().selected_stamp(),
        &operations,
    );
    let preconditions = bind(
        BufferCarrierRole::Preconditions,
        descriptor.carriers().preconditions().selected_stamp(),
        &[expected],
    );
    let BufferPlanBindingOutcome::Accepted(plan) =
        descriptor.bind_plan(stamp, *target, origin, operations, preconditions)
    else {
        panic!("selected byte-native operation")
    };
    plan
}

fn observation(model: &ByteModel) -> (MemoryResource, ResourceObservedState) {
    let mut resource = MemoryResource::new(
        &model.binding(),
        ResourceAccessStamp::new(ResourceIdentity::new(6), life(7)),
        b"# a\n\xff".to_vec(),
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

fn preparation<'a>(
    model: &ByteModel,
    input: ResourceStateInput<'a, ByteState>,
) -> ResourceStatePreparation<'a, ByteState> {
    ResourceStatePreparation::new(
        model.binding(),
        input,
        limits(),
        CancellationState::Open,
        DeadlineState::Pending,
    )
    .unwrap()
}

fn limits() -> ResourceStateLimits {
    ResourceStateLimits {
        resident_bytes: profile::bytes(4096),
        work_units: profile::count(4096),
        observation_count: profile::count(1),
        observation_bytes: profile::bytes(4096),
    }
}

const fn life(id: u128) -> LifecycleIdentity {
    LifecycleIdentity::new(id, 1, Generation::new(1))
}
