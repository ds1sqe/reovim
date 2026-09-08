use reovim_kernel_interface::cancellation::{CancellationState, DeadlineState};
use reovim_kernel_interface::lifecycle::{Generation, LifecycleIdentity};
use reovim_kernel_interface::resource::delivery::ResourceAdmissionStatus;
use reovim_kernel_interface::resource::port::ResourceAccessPort;
use reovim_kernel_interface::resource::state::{
    ResourceStateError, ResourceStateInput, ResourceStateLimits,
    ResourceStateModel, ResourceStatePreparation,
};
use reovim_kernel_interface::resource::{
    ResourceAccessStamp, ResourceIdentity,
};
use reovim_resource_local::{
    ByteModel, Key, KeyedModel, MemoryResource, profile,
};

#[test]
fn byte_native_initial_state_does_not_decode_or_normalize_text() {
    let model = ByteModel::new(life(1), life(2));
    let state = initial(&model, b"# a\n\xff\0", limits(128, 128))
        .expect("arbitrary source bytes");
    assert_eq!(state.as_bytes(), b"# a\n\xff\0");
}

#[test]
fn keyed_identity_is_independent_of_source_order_and_missing_is_not_false() {
    let model = KeyedModel::new(life(1), life(2));
    let first = initial(&model, &[17, 0, 93, 1], limits(128, 128)).unwrap();
    let reversed = initial(&model, &[93, 1, 17, 0], limits(128, 128)).unwrap();
    assert_eq!(first, reversed);
    assert_eq!(first.get(Key::Alpha), Some(false));
    assert_eq!(first.get(Key::Omega), Some(true));
    let sparse = initial(&model, &[93, 0], limits(128, 128)).unwrap();
    assert_eq!(sparse.get(Key::Alpha), None);
    assert_eq!(sparse.get(Key::Omega), Some(false));
    let empty = initial(&model, &[], limits(128, 128)).unwrap();
    assert_eq!(empty.get(Key::Alpha), None);
    assert_eq!(empty.get(Key::Omega), None);
}

#[test]
fn ambiguous_or_invalid_keyed_source_is_rejected_not_silently_repaired() {
    let model = KeyedModel::new(life(1), life(2));
    for invalid in [&[17, 0, 17, 1][..], &[93, 2], &[42, 0], &[17]] {
        assert_eq!(
            initial(&model, invalid, limits(128, 128)),
            Err(ResourceStateError::MalformedOperation)
        );
    }
}

#[test]
fn native_preparation_obeys_its_work_and_resident_reservations() {
    let model = ByteModel::new(life(1), life(2));
    assert_eq!(
        initial(&model, b"# a\n\xff", limits(128, 1)),
        Err(ResourceStateError::Pressure)
    );
    assert_eq!(
        initial(&model, b"# a\n\xff", limits(1, 128)),
        Err(ResourceStateError::Pressure)
    );
    let keyed = KeyedModel::new(life(1), life(2));
    assert_eq!(
        initial(&keyed, &[17, 0], limits(128, 1)),
        Err(ResourceStateError::Pressure)
    );
}

fn initial<Model: ResourceStateModel>(
    model: &Model,
    bytes: &[u8],
    limits: ResourceStateLimits,
) -> Result<Model::State, ResourceStateError> {
    let mut resource = MemoryResource::new(
        &model.binding(),
        ResourceAccessStamp::new(ResourceIdentity::new(3), life(4)),
        bytes.to_vec(),
    )
    .expect("bounded native source");
    let open = resource.open_request(1);
    assert_eq!(
        resource.submit_open(open).status(),
        ResourceAdmissionStatus::Admitted
    );
    resource
        .take_open_completion(open.operation())
        .into_completion()
        .expect("initial source Ready");
    let observation = resource.observation_request(2);
    let operation = observation.operation();
    assert_eq!(
        resource.submit_observation(observation).status(),
        ResourceAdmissionStatus::Admitted
    );
    let observed = resource
        .take_observation_completion(operation)
        .into_completion()
        .expect("source completion")
        .into_parts()
        .1
        .into_observed_state()
        .expect("native source data");
    let preparation = ResourceStatePreparation::new(
        model.binding(),
        ResourceStateInput::Initial(&observed),
        limits,
        CancellationState::Open,
        DeadlineState::Pending,
    )?;
    preparation
        .prepare_with(model)
        .map(|candidate| candidate.into_output().state)
        .map_err(|rejected| rejected.cause)
}

const fn life(id: u128) -> LifecycleIdentity {
    LifecycleIdentity::new(id, id, Generation::new(1))
}

fn limits(resident: u64, work: u64) -> ResourceStateLimits {
    ResourceStateLimits {
        resident_bytes: profile::bytes(resident),
        work_units: profile::count(work),
        observation_count: profile::count(1),
        observation_bytes: profile::bytes(128),
    }
}
