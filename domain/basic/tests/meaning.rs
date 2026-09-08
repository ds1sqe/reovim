use reovim_domain_basic::{Heading, heading, replacement, selected_key};
use reovim_kernel_interface::cancellation::{CancellationState, DeadlineState};
use reovim_kernel_interface::lifecycle::{Generation, LifecycleIdentity};
use reovim_kernel_interface::resource::port::ResourceAccessPort;
use reovim_kernel_interface::resource::state::{
    ResourceStateInput, ResourceStateLimits, ResourceStateModel,
    ResourceStatePreparation,
};
use reovim_kernel_interface::resource::{
    ResourceAccessStamp, ResourceIdentity,
};
use reovim_kernel_interface::semantic::policy::SemanticPolicyFailure;
use reovim_resource_local::{
    ByteModel, Key, KeyedModel, MemoryResource, profile,
};

#[test]
fn heading_correspondence_reads_native_structure_not_only_stamps() {
    let model = ByteModel::new(life(1), life(2));
    let shallow = initial(&model, b"# a\n\xff");
    let deeper = initial(&model, b"## b\n\xff");
    assert_eq!(
        heading(&shallow),
        Ok(Heading { level: 1, offset: 2, label: b'a' })
    );
    assert_eq!(
        heading(&deeper),
        Ok(Heading { level: 2, offset: 3, label: b'b' })
    );
    assert_eq!(shallow.as_bytes(), b"# a\n\xff");
    assert_eq!(deeper.as_bytes(), b"## b\n\xff");
}

#[test]
fn missing_heading_and_unrepresentable_title_are_distinct_failures() {
    let model = ByteModel::new(life(1), life(2));
    for source in [&b""[..], b"ordinary source", b"### c\n"] {
        assert_eq!(
            heading(&initial(&model, source)),
            Err(SemanticPolicyFailure::NoCorrespondence)
        );
    }
    assert_eq!(
        heading(&initial(&model, b"# \xff\n")),
        Err(SemanticPolicyFailure::Unrepresentable)
    );
    assert_eq!(replacement(b"z"), Ok(b'z'));
    assert_eq!(replacement(b"zz"), Err(SemanticPolicyFailure::InvalidIntent));
    assert_eq!(
        replacement(&[0xff]),
        Err(SemanticPolicyFailure::Unrepresentable)
    );
}

#[test]
fn toggle_meaning_uses_named_current_value_and_rejects_absent_records() {
    let model = KeyedModel::new(life(1), life(2));
    let disabled = initial(&model, &[93, 1, 17, 0]);
    let enabled = initial(&model, &[17, 1, 93, 1]);
    let sparse = initial(&model, &[93, 1]);
    assert_eq!(selected_key(&disabled, &[17]), Ok((Key::Alpha, false)));
    assert_eq!(selected_key(&enabled, &[17]), Ok((Key::Alpha, true)));
    assert_eq!(
        selected_key(&sparse, &[17]),
        Err(SemanticPolicyFailure::NoCorrespondence)
    );
    assert_eq!(selected_key(&sparse, &[93]), Ok((Key::Omega, true)));
    assert_eq!(
        selected_key(&enabled, &[42]),
        Err(SemanticPolicyFailure::InvalidIntent)
    );
}

fn initial<Model: ResourceStateModel>(
    model: &Model,
    bytes: &[u8],
) -> Model::State {
    let mut source = MemoryResource::new(
        &model.binding(),
        ResourceAccessStamp::new(ResourceIdentity::new(3), life(4)),
        bytes.to_vec(),
    )
    .expect("bounded native data");
    let open = source.open_request(1);
    source.submit_open(open);
    source
        .take_open_completion(open.operation())
        .into_completion()
        .expect("source Ready");
    let request = source.observation_request(2);
    let operation = request.operation();
    source.submit_observation(request);
    let observed = source
        .take_observation_completion(operation)
        .into_completion()
        .expect("observation completed")
        .into_parts()
        .1
        .into_observed_state()
        .expect("actual native state");
    let request = ResourceStatePreparation::new(
        model.binding(),
        ResourceStateInput::Initial(&observed),
        ResourceStateLimits {
            resident_bytes: profile::bytes(128),
            work_units: profile::count(128),
            observation_count: profile::count(1),
            observation_bytes: profile::bytes(128),
        },
        CancellationState::Open,
        DeadlineState::Pending,
    )
    .expect("selected native preparation");
    match request.prepare_with(model) {
        Ok(candidate) => candidate.into_output().state,
        Err(failure) => panic!("native preparation: {:?}", failure.cause),
    }
}

const fn life(id: u128) -> LifecycleIdentity {
    LifecycleIdentity::new(id, id, Generation::new(1))
}
