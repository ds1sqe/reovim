use reovim_kernel_interface::lifecycle::{Generation, LifecycleIdentity};
use reovim_kernel_interface::resource::access::{
    ResourceCloseOutcome, ResourceObservationFailure,
};
use reovim_kernel_interface::resource::delivery::{
    ResourceAdmissionFailure, ResourceAdmissionStatus,
    ResourceCancellationResult, ResourceCompletionStatus,
    ResourceCompletionUnavailable,
};
use reovim_kernel_interface::resource::operation::ResourceCancellationRequest;
use reovim_kernel_interface::resource::port::ResourceAccessPort;
use reovim_kernel_interface::resource::state::ResourceStateModel;
use reovim_kernel_interface::resource::{
    ResourceAccessStamp, ResourceIdentity,
};
use reovim_resource_local::{ByteModel, MemoryResource, MemoryResourceError};

fn source(bytes: &[u8]) -> MemoryResource {
    let model = ByteModel::new(lifecycle(1), lifecycle(2));
    MemoryResource::new(
        &model.binding(),
        ResourceAccessStamp::new(ResourceIdentity::new(9), lifecycle(3)),
        bytes.to_vec(),
    )
    .expect("bounded source")
}

const fn lifecycle(id: u128) -> LifecycleIdentity {
    LifecycleIdentity::new(id, id, Generation::new(1))
}

fn open(source: &mut MemoryResource) {
    let request = source.open_request(1);
    assert_eq!(
        source.submit_open(request).status(),
        ResourceAdmissionStatus::Admitted
    );
    let completion = source
        .take_open_completion(request.operation())
        .into_completion()
        .expect("open result");
    assert_eq!(
        completion.outcome().ready_state().expect("Ready").snapshot(),
        source.snapshot()
    );
}

#[test]
fn source_observation_preserves_invalid_units_and_exact_identity() {
    let mut source = source(b"# a\n\xff\0");
    open(&mut source);
    let request = source.observation_request(2);
    let operation = request.operation();
    assert_eq!(
        source.submit_observation(request).status(),
        ResourceAdmissionStatus::Admitted
    );
    let completion = source
        .take_observation_completion(operation)
        .into_completion()
        .expect("observation result");
    let observed = completion
        .into_parts()
        .1
        .into_observed_state()
        .expect("actual source observation");
    assert_eq!(observed.payload().bytes(), b"# a\n\xff\0");
    assert_eq!(observed.snapshot(), source.snapshot());
    assert_eq!(
        source.take_observation_completion(operation).status(),
        ResourceCompletionStatus::Unavailable(
            ResourceCompletionUnavailable::AlreadyDelivered,
        )
    );
}

#[test]
fn close_preserves_terminal_observation_until_delivered() {
    let mut source = source(b"native state");
    open(&mut source);
    let request = source.observation_request(2);
    let operation = request.operation();
    source.submit_observation(request);
    let close = source.close_request();
    assert!(matches!(source.close(close), ResourceCloseOutcome::Incomplete(_)));
    let refused = source.submit_open(source.open_request(3));
    assert_eq!(refused.failure(), Some(ResourceAdmissionFailure::Closing));
    let observed = source
        .take_observation_completion(operation)
        .into_completion()
        .expect("retained terminal")
        .into_parts()
        .1
        .into_observed_state()
        .expect("preserved payload");
    assert_eq!(observed.payload().bytes(), b"native state");
    assert_eq!(source.close(close), ResourceCloseOutcome::Closed);
    assert_eq!(source.close(close), ResourceCloseOutcome::Closed);
    assert_eq!(
        source.submit_open(source.open_request(4)).failure(),
        Some(ResourceAdmissionFailure::Closed)
    );
}

#[test]
fn empty_native_source_is_present_not_unavailable() {
    let mut source = source(b"");
    open(&mut source);
    let request = source.observation_request(2);
    let operation = request.operation();
    source.submit_observation(request);
    let completion = source
        .take_observation_completion(operation)
        .into_completion()
        .expect("observed empty state");
    assert_ne!(
        completion.outcome().failure(),
        Some(ResourceObservationFailure::Unavailable)
    );
    assert_eq!(
        completion
            .outcome()
            .observed_state()
            .expect("present source")
            .payload()
            .bytes(),
        b""
    );
}

#[test]
fn cancellation_after_bounded_execution_cannot_erase_terminal_data() {
    use reovim_kernel_interface::cancellation::CancellationCause;

    let mut source = source(b"retained");
    let request = source.open_request(1);
    source.submit_open(request);
    let cancellation = ResourceCancellationRequest::new(
        request.operation(),
        CancellationCause::ExplicitRequest,
    )
    .expect("open cancellation");
    assert_eq!(
        source.request_cancellation(cancellation),
        ResourceCancellationResult::AlreadyTerminal
    );
    assert!(
        source
            .take_open_completion(request.operation())
            .into_completion()
            .expect("terminal retained")
            .outcome()
            .ready_state()
            .is_some()
    );
}

#[test]
fn schema_mismatch_is_rejected_before_observation() {
    let model = ByteModel::new(lifecycle(1), lifecycle(2));
    let mut binding = model.binding();
    binding.payload =
        reovim_kernel_interface::resource::carrier::ResourceCarrierStamp::new(
            0x1005, 2,
        );
    let result = MemoryResource::new(
        &binding,
        ResourceAccessStamp::new(ResourceIdentity::new(9), lifecycle(3)),
        b"source".to_vec(),
    );
    assert!(matches!(result, Err(MemoryResourceError::UnsupportedModel)));
}

#[test]
fn duplicate_open_returns_the_original_request_without_new_work() {
    let mut source = source(b"one source");
    let request = source.open_request(1);
    source.submit_open(request);
    let refused = source.submit_open(request);
    assert_eq!(
        refused.failure(),
        Some(ResourceAdmissionFailure::DuplicateRequest)
    );
    assert_eq!(refused.into_returned_request(), Some(request));
    let completion = source
        .take_open_completion(request.operation())
        .into_completion()
        .expect("original open result");
    assert_eq!(completion.request(), request);
}
