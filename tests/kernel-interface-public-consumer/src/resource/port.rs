use reovim_kernel_interface::bound::{CheckedCount, ZeroPolicy};
use reovim_kernel_interface::cancellation::{
    CancellationCause, CancellationOutcome, ExecutionOutcome,
};
use reovim_kernel_interface::lifecycle::{Generation, LifecycleIdentity};
use reovim_kernel_interface::resource::access::{
    ResourceAccessDescriptor, ResourceApplicabilityOutcome,
    ResourceChangeCheckpoint, ResourceCloseOutcome, ResourceIncompleteClose,
    ResourceObservationFailure, ResourceObservationOutcome,
    ResourceObservedState, ResourceObservedStateError, ResourceOpenOutcome,
    ResourceOpenStatus, ResourceReadyState,
};
use reovim_kernel_interface::resource::carrier::ResourcePayloadCarrier;
use reovim_kernel_interface::resource::delivery::{
    ResourceAdmissionFailure, ResourceAdmissionStatus,
    ResourceCancellationResult, ResourceCompletionStatus,
    ResourceCompletionUnavailable, ResourceObservationAdmission,
    ResourceObservationCompletion, ResourceObservationCompletionDelivery,
    ResourceOpenAdmission, ResourceOpenCompletion,
    ResourceOpenCompletionDelivery,
};
use reovim_kernel_interface::resource::operation::{
    ResourceCancellationIdentity, ResourceCancellationRequest,
    ResourceCloseRequest, ResourceCorrelationIdentity, ResourceDeadline,
    ResourceDeadlineIdentity, ResourceObservationRequest,
    ResourceObservationTarget, ResourceOpenRequest, ResourceOperationKind,
    ResourceOperationStamp, ResourceRequestError, ResourceRequestIdentity,
};
use reovim_kernel_interface::resource::port::ResourceAccessPort;
use reovim_kernel_interface::resource::{
    ResourceIdentity, ResourceSnapshotIdentity, ResourceSnapshotStamp,
};

use super::snapshot;
use crate::traits;

struct ScriptedResourcePort {
    descriptor: ResourceAccessDescriptor,
    open_pending: Option<ResourceOpenRequest>,
    open_completion: Option<ResourceOpenCompletion>,
    open_delivered: Option<ResourceOperationStamp>,
    observation_pending: Option<ResourceObservationRequest>,
    observation_completion: Option<ResourceObservationCompletion>,
    observation_delivered: Option<ResourceOperationStamp>,
    cancellation: Option<ResourceOperationStamp>,
    closed: bool,
}

impl ScriptedResourcePort {
    const fn new(descriptor: ResourceAccessDescriptor) -> Self {
        Self {
            descriptor,
            open_pending: None,
            open_completion: None,
            open_delivered: None,
            observation_pending: None,
            observation_completion: None,
            observation_delivered: None,
            cancellation: None,
            closed: false,
        }
    }

    const fn complete_open(&mut self, outcome: ResourceOpenOutcome) -> bool {
        let Some(request) = self.open_pending.take() else {
            return false;
        };
        self.open_completion =
            Some(ResourceOpenCompletion::new(request, outcome));
        true
    }

    fn complete_observation(
        &mut self,
        outcome: ResourceObservationOutcome,
    ) -> bool {
        let Some(request) = self.observation_pending.take() else {
            return false;
        };
        self.observation_completion =
            Some(ResourceObservationCompletion::new(request, outcome));
        true
    }

    const fn operation_is_current(
        &self,
        operation: ResourceOperationStamp,
    ) -> bool {
        self.descriptor.adapter().same_generation(operation.adapter())
    }

    const fn has_open_work(&self) -> bool {
        self.open_pending.is_some() || self.open_completion.is_some()
    }

    const fn has_observation_work(&self) -> bool {
        self.observation_pending.is_some()
            || self.observation_completion.is_some()
    }

    const fn deliver_open(
        &mut self,
        operation: ResourceOperationStamp,
    ) -> ResourceOpenCompletionDelivery {
        let Some(completion) = self.open_completion.take() else {
            return open_unavailable(ResourceCompletionUnavailable::Gap);
        };
        self.open_delivered = Some(operation);
        ResourceOpenCompletionDelivery::delivered(completion)
    }

    fn deliver_observation(
        &mut self,
        operation: ResourceOperationStamp,
    ) -> ResourceObservationCompletionDelivery {
        let Some(completion) = self.observation_completion.take() else {
            return observation_unavailable(ResourceCompletionUnavailable::Gap);
        };
        self.observation_delivered = Some(operation);
        ResourceObservationCompletionDelivery::delivered(completion)
    }
}

impl ::reovim_kernel_interface::resource::port::ResourceAccessPort
    for ScriptedResourcePort
{
    fn descriptor(&self) -> &ResourceAccessDescriptor {
        &self.descriptor
    }

    fn applicability(
        &self,
        request: &ResourceOpenRequest,
    ) -> ResourceApplicabilityOutcome {
        if self.closed {
            return ResourceApplicabilityOutcome::Incompatible;
        }
        if request.resource().value() == 0 {
            return ResourceApplicabilityOutcome::Denied;
        }
        if request.resource().value() == 11 {
            return ResourceApplicabilityOutcome::Applicable;
        }
        ResourceApplicabilityOutcome::Unsupported
    }

    fn submit_open(
        &mut self,
        request: ResourceOpenRequest,
    ) -> ResourceOpenAdmission {
        if self.closed {
            return ResourceOpenAdmission::not_admitted(
                ResourceAdmissionFailure::Closed,
                request,
            );
        }
        if !self.operation_is_current(request.operation()) {
            return ResourceOpenAdmission::not_admitted(
                ResourceAdmissionFailure::Replaced,
                request,
            );
        }
        if self.has_open_work() {
            return ResourceOpenAdmission::not_admitted(
                ResourceAdmissionFailure::Pressure,
                request,
            );
        }
        let admission = ResourceOpenAdmission::admitted(&request);
        self.open_pending = Some(request);
        admission
    }

    fn submit_observation(
        &mut self,
        request: ResourceObservationRequest,
    ) -> ResourceObservationAdmission {
        if self.closed {
            return ResourceObservationAdmission::not_admitted(
                ResourceAdmissionFailure::Closed,
                request,
            );
        }
        if !self.operation_is_current(request.operation()) {
            return ResourceObservationAdmission::not_admitted(
                ResourceAdmissionFailure::Replaced,
                request,
            );
        }
        if self.has_observation_work() {
            return ResourceObservationAdmission::not_admitted(
                ResourceAdmissionFailure::Pressure,
                request,
            );
        }
        let admission = ResourceObservationAdmission::admitted(&request);
        self.observation_pending = Some(request);
        admission
    }

    fn request_cancellation(
        &mut self,
        request: ResourceCancellationRequest,
    ) -> ResourceCancellationResult {
        let operation = request.operation();
        if self.closed {
            return ResourceCancellationResult::Closed;
        }
        if !self.operation_is_current(operation) {
            return ResourceCancellationResult::StaleAdapter;
        }
        if self.cancellation == Some(operation) {
            return ResourceCancellationResult::AlreadyRequested(
                CancellationOutcome::new(
                    request.cause(),
                    ExecutionOutcome::OwnedNonterminal,
                ),
            );
        }
        if pending_operation(self.open_pending.as_ref()) == Some(operation)
            || pending_observation(self.observation_pending.as_ref())
                == Some(operation)
        {
            self.cancellation = Some(operation);
            return ResourceCancellationResult::Requested(
                CancellationOutcome::new(
                    request.cause(),
                    ExecutionOutcome::OwnedNonterminal,
                ),
            );
        }
        if completion_operation(self.open_completion.as_ref())
            == Some(operation)
            || observation_completion_operation(
                self.observation_completion.as_ref(),
            ) == Some(operation)
        {
            return ResourceCancellationResult::AlreadyTerminal;
        }
        ResourceCancellationResult::UnknownRequest
    }

    fn take_open_completion(
        &mut self,
        operation: ResourceOperationStamp,
    ) -> ResourceOpenCompletionDelivery {
        if operation.kind() != ResourceOperationKind::Open {
            return open_unavailable(
                ResourceCompletionUnavailable::WrongOperation,
            );
        }
        if !self.operation_is_current(operation) {
            return open_unavailable(ResourceCompletionUnavailable::Replaced);
        }
        if completion_operation(self.open_completion.as_ref())
            == Some(operation)
        {
            return self.deliver_open(operation);
        }
        if pending_operation(self.open_pending.as_ref()) == Some(operation) {
            return open_unavailable(ResourceCompletionUnavailable::Pending);
        }
        if self.open_delivered == Some(operation) {
            return open_unavailable(
                ResourceCompletionUnavailable::AlreadyDelivered,
            );
        }
        open_unavailable(ResourceCompletionUnavailable::UnknownRequest)
    }

    fn take_observation_completion(
        &mut self,
        operation: ResourceOperationStamp,
    ) -> ResourceObservationCompletionDelivery {
        if operation.kind() != ResourceOperationKind::Observation {
            return observation_unavailable(
                ResourceCompletionUnavailable::WrongOperation,
            );
        }
        if !self.operation_is_current(operation) {
            return observation_unavailable(
                ResourceCompletionUnavailable::Replaced,
            );
        }
        if observation_completion_operation(
            self.observation_completion.as_ref(),
        ) == Some(operation)
        {
            return self.deliver_observation(operation);
        }
        if pending_observation(self.observation_pending.as_ref())
            == Some(operation)
        {
            return observation_unavailable(
                ResourceCompletionUnavailable::Pending,
            );
        }
        if self.observation_delivered == Some(operation) {
            return observation_unavailable(
                ResourceCompletionUnavailable::AlreadyDelivered,
            );
        }
        observation_unavailable(ResourceCompletionUnavailable::UnknownRequest)
    }

    fn close(&mut self, request: ResourceCloseRequest) -> ResourceCloseOutcome {
        if !self.operation_is_current(request.operation()) {
            return ResourceCloseOutcome::StaleAdapter;
        }
        if self.open_pending.is_some() || self.observation_pending.is_some() {
            return incomplete_close();
        }
        self.closed = true;
        ResourceCloseOutcome::Closed
    }
}

fn incomplete_close() -> ResourceCloseOutcome {
    let Some(unresolved) = CheckedCount::new(1, ZeroPolicy::Reject).ok() else {
        return ResourceCloseOutcome::StaleAdapter;
    };
    let Some(incomplete) = ResourceIncompleteClose::new(
        ExecutionOutcome::OwnedNonterminal,
        unresolved,
    )
    .ok() else {
        return ResourceCloseOutcome::StaleAdapter;
    };
    ResourceCloseOutcome::Incomplete(incomplete)
}

pub const fn const_cases() -> bool {
    derive_identity_cases()
        && derive_request_cases()
        && derive_delivery_cases()
        && identity_fields()
        && operation_kind_cases()
        && deadline_cases()
}

pub fn exercise() -> bool {
    let Some(descriptor) = super::access::stable_descriptor() else {
        return false;
    };
    let mut port = ScriptedResourcePort::new(descriptor);
    variant_cases()
        && descriptor_and_applicability_cases(&port)
        && request_validation_cases(&port.descriptor)
        && observed_state_validation_cases(&port.descriptor)
        && completion_lookup_cases(&mut port)
        && open_journey(&mut port)
        && observation_journey(&mut port)
        && cancellation_and_close_journey(&mut port)
}

const fn derive_identity_cases() -> bool {
    crate::derive_witness::<
        ::reovim_kernel_interface::resource::operation::
            ResourceRequestIdentity,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::resource::operation::
            ResourceCancellationIdentity,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::resource::operation::
            ResourceCorrelationIdentity,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::resource::operation::
            ResourceDeadlineIdentity,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::resource::operation::ResourceDeadline,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::resource::operation::ResourceOperationKind,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::resource::operation::
            ResourceOperationStamp,
    >()
}

const fn derive_request_cases() -> bool {
    crate::derive_witness::<
        ::reovim_kernel_interface::resource::operation::ResourceRequestError,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::resource::operation::ResourceOpenRequest,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::resource::operation::
            ResourceObservationTarget,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::resource::operation::
            ResourceCancellationRequest,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::resource::operation::ResourceCloseRequest,
    >() && crate::send_sync_witness::<
        ::reovim_kernel_interface::resource::operation::
            ResourceObservationRequest,
    >()
}

const fn derive_delivery_cases() -> bool {
    crate::derive_witness::<
        ::reovim_kernel_interface::resource::delivery::
            ResourceAdmissionStatus,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::resource::delivery::
            ResourceAdmissionFailure,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::resource::delivery::
            ResourceCancellationResult,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::resource::delivery::
            ResourceCompletionUnavailable,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::resource::delivery::
            ResourceCompletionStatus,
    >() && crate::send_sync_witness::<
        ::reovim_kernel_interface::resource::delivery::ResourceOpenAdmission,
    >() && crate::send_sync_witness::<
        ::reovim_kernel_interface::resource::delivery::
            ResourceObservationAdmission,
    >() && crate::send_sync_witness::<
        ::reovim_kernel_interface::resource::delivery::ResourceOpenCompletion,
    >() && crate::send_sync_witness::<
        ::reovim_kernel_interface::resource::delivery::
            ResourceObservationCompletion,
    >() && crate::send_sync_witness::<
        ::reovim_kernel_interface::resource::delivery::
            ResourceOpenCompletionDelivery,
    >() && crate::send_sync_witness::<
        ::reovim_kernel_interface::resource::delivery::
            ResourceObservationCompletionDelivery,
    >()
}

fn variant_cases() -> bool {
    request_error_cases()
        && admission_failure_cases()
        && completion_unavailable_cases()
        && cancellation_result_cases()
}

const fn identity_fields() -> bool {
    ResourceRequestIdentity::new(1).value() == 1
        && ResourceCancellationIdentity::new(2).value() == 2
        && ResourceCorrelationIdentity::new(3).value() == 3
        && ResourceDeadlineIdentity::new(4).value() == 4
}

const fn operation_kind_cases() -> bool {
    operation_kind_code(ResourceOperationKind::Open) == 0
        && operation_kind_code(ResourceOperationKind::Observation) == 1
        && operation_kind_code(ResourceOperationKind::Close) == 2
}

const fn operation_kind_code(kind: ResourceOperationKind) -> u8 {
    match kind {
        ResourceOperationKind::Open => 0,
        ResourceOperationKind::Observation => 1,
        ResourceOperationKind::Close => 2,
    }
}

const fn deadline_cases() -> bool {
    deadline_code(ResourceDeadline::None) == 0
        && deadline_code(ResourceDeadline::Governed(
            ResourceDeadlineIdentity::new(4),
        )) == 4
}

const fn deadline_code(deadline: ResourceDeadline) -> u128 {
    match deadline {
        ResourceDeadline::None => 0,
        ResourceDeadline::Governed(identity) => identity.value(),
    }
}

fn request_error_cases() -> bool {
    traits(ResourceRequestError::AdapterGenerationMismatch)
        && traits(ResourceRequestError::OperationKindMismatch {
            expected: ResourceOperationKind::Open,
            actual: ResourceOperationKind::Close,
        })
        && traits(ResourceRequestError::CloseIsNotCancellable)
}

fn admission_failure_cases() -> bool {
    let values = [
        ResourceAdmissionFailure::Denied,
        ResourceAdmissionFailure::Unsupported,
        ResourceAdmissionFailure::Incompatible,
        ResourceAdmissionFailure::InvalidRequest,
        ResourceAdmissionFailure::NotReady,
        ResourceAdmissionFailure::DuplicateRequest,
        ResourceAdmissionFailure::StaleAccess,
        ResourceAdmissionFailure::Pressure,
        ResourceAdmissionFailure::Cancelled,
        ResourceAdmissionFailure::Replaced,
        ResourceAdmissionFailure::Closing,
        ResourceAdmissionFailure::Closed,
        ResourceAdmissionFailure::AdapterFailure,
    ];
    let mut index = 0;
    while index < values.len() {
        if !traits(values[index]) {
            return false;
        }
        index += 1;
    }
    true
}

fn completion_unavailable_cases() -> bool {
    let values = [
        ResourceCompletionUnavailable::Pending,
        ResourceCompletionUnavailable::UnknownRequest,
        ResourceCompletionUnavailable::WrongOperation,
        ResourceCompletionUnavailable::AlreadyDelivered,
        ResourceCompletionUnavailable::Gap,
        ResourceCompletionUnavailable::Replaced,
        ResourceCompletionUnavailable::Closing,
        ResourceCompletionUnavailable::Closed,
    ];
    let mut index = 0;
    while index < values.len() {
        if !traits(values[index]) {
            return false;
        }
        index += 1;
    }
    true
}

fn cancellation_result_cases() -> bool {
    let stopped = CancellationOutcome::new(
        CancellationCause::ExplicitRequest,
        ExecutionOutcome::Stopped,
    );
    traits(ResourceCancellationResult::Requested(stopped))
        && traits(ResourceCancellationResult::AlreadyRequested(stopped))
        && traits(ResourceCancellationResult::AlreadyTerminal)
        && traits(ResourceCancellationResult::Denied)
        && traits(ResourceCancellationResult::UnknownRequest)
        && traits(ResourceCancellationResult::StaleAdapter)
        && traits(ResourceCancellationResult::Closing)
        && traits(ResourceCancellationResult::Closed)
}

fn descriptor_and_applicability_cases(port: &ScriptedResourcePort) -> bool {
    let descriptor = ResourceAccessPort::descriptor(port);
    let Some(applicable) = open_request(descriptor, 1, 11) else {
        return false;
    };
    let Some(denied) = open_request(descriptor, 2, 0) else {
        return false;
    };
    let Some(unsupported) = open_request(descriptor, 3, 99) else {
        return false;
    };
    ResourceAccessPort::applicability(port, &applicable)
        == ResourceApplicabilityOutcome::Applicable
        && ResourceAccessPort::applicability(port, &denied)
            == ResourceApplicabilityOutcome::Denied
        && ResourceAccessPort::applicability(port, &unsupported)
            == ResourceApplicabilityOutcome::Unsupported
}

const fn request_validation_cases(
    descriptor: &ResourceAccessDescriptor,
) -> bool {
    let wrong_kind = ResourceOpenRequest::new(
        descriptor,
        operation(descriptor, ResourceOperationKind::Observation, 4),
        ResourceIdentity::new(11),
    );
    let stale = ResourceOpenRequest::new(
        descriptor,
        stale_operation(descriptor, ResourceOperationKind::Open, 5),
        ResourceIdentity::new(11),
    );
    let close_cancellation = ResourceCancellationRequest::new(
        operation(descriptor, ResourceOperationKind::Close, 6),
        CancellationCause::ExplicitRequest,
    );
    request_error_code(wrong_kind) == 1
        && request_error_code(stale) == 2
        && cancellation_error_code(close_cancellation) == 1
        && error_traits_for_request()
}

const fn request_error_code(
    result: Result<ResourceOpenRequest, ResourceRequestError>,
) -> u8 {
    match result {
        Err(ResourceRequestError::OperationKindMismatch { .. }) => 1,
        Err(ResourceRequestError::AdapterGenerationMismatch) => 2,
        Err(ResourceRequestError::CloseIsNotCancellable) | Ok(_) => 0,
    }
}

const fn cancellation_error_code(
    result: Result<ResourceCancellationRequest, ResourceRequestError>,
) -> u8 {
    match result {
        Err(ResourceRequestError::CloseIsNotCancellable) => 1,
        Err(
            ResourceRequestError::AdapterGenerationMismatch
            | ResourceRequestError::OperationKindMismatch { .. },
        )
        | Ok(_) => 0,
    }
}

const fn error_traits_for_request() -> bool {
    crate::error_traits(&ResourceRequestError::AdapterGenerationMismatch)
}

fn observed_state_validation_cases(
    descriptor: &ResourceAccessDescriptor,
) -> bool {
    let Some(actual) = snapshot(1, 14) else { return false };
    let Some(stale) = snapshot(2, 14) else { return false };
    let historical = ResourceSnapshotStamp::new(
        actual.access(),
        actual.revision(),
        ResourceSnapshotIdentity::new(16),
    );
    let Some(one) = CheckedCount::new(1, ZeroPolicy::Reject).ok() else {
        return false;
    };
    let Some(two) = CheckedCount::new(2, ZeroPolicy::Reject).ok() else {
        return false;
    };
    let Some(access_payload) = payload_carrier(descriptor) else {
        return false;
    };
    let access = ResourceObservedState::new(
        descriptor,
        ResourceObservationTarget::Current(stale.access()),
        one,
        actual,
        access_payload,
    );
    let Some(snapshot_payload) = payload_carrier(descriptor) else {
        return false;
    };
    let exact = ResourceObservedState::new(
        descriptor,
        ResourceObservationTarget::Snapshot(historical),
        one,
        actual,
        snapshot_payload,
    );
    let Some(region_payload) = payload_carrier(descriptor) else {
        return false;
    };
    let regions = ResourceObservedState::new(
        descriptor,
        ResourceObservationTarget::Current(actual.access()),
        two,
        actual,
        region_payload,
    );
    observed_error_code(access) == 1
        && observed_error_code(exact) == 2
        && observed_error_code(regions) == 3
}

fn payload_carrier(
    descriptor: &ResourceAccessDescriptor,
) -> Option<ResourcePayloadCarrier> {
    let stamp = descriptor.carriers().payload().selected_stamp();
    let prepared = descriptor.prepare_payload_carrier(stamp, 1, 5).ok()?;
    descriptor.bind_payload_carrier(prepared, b"hello".as_slice().into()).ok()
}

fn observed_error_code(
    result: Result<
        ResourceObservedState,
        (ResourceObservedStateError, ResourcePayloadCarrier),
    >,
) -> u8 {
    match result {
        Err((ResourceObservedStateError::AccessMismatch, payload)) => {
            u8::from(payload.bytes() == b"hello")
        }
        Err((ResourceObservedStateError::SnapshotMismatch, payload)) => {
            2 * u8::from(payload.bytes() == b"hello")
        }
        Err((ResourceObservedStateError::RegionCountMismatch, payload)) => {
            3 * u8::from(payload.bytes() == b"hello")
        }
        Err((
            ResourceObservedStateError::RevisionSchemaMismatch
            | ResourceObservedStateError::Carrier(_),
            _,
        ))
        | Ok(_) => 0,
    }
}

fn completion_lookup_cases(port: &mut ScriptedResourcePort) -> bool {
    let wrong =
        operation(&port.descriptor, ResourceOperationKind::Observation, 7);
    let stale =
        stale_operation(&port.descriptor, ResourceOperationKind::Open, 8);
    completion_is(
        ResourceAccessPort::take_open_completion(port, wrong).status(),
        ResourceCompletionUnavailable::WrongOperation,
    ) && completion_is(
        ResourceAccessPort::take_open_completion(port, stale).status(),
        ResourceCompletionUnavailable::Replaced,
    )
}

fn open_journey(port: &mut ScriptedResourcePort) -> bool {
    let Some(request) = open_request(&port.descriptor, 10, 11) else {
        return false;
    };
    let operation = request.operation();
    let admission = ResourceAccessPort::submit_open(port, request);
    if admission.status() != ResourceAdmissionStatus::Admitted
        || admission.admitted_operation() != Some(operation)
        || admission.failure().is_some()
        || admission.returned_request().is_some()
    {
        return false;
    }
    if !completion_is(
        ResourceAccessPort::take_open_completion(port, operation).status(),
        ResourceCompletionUnavailable::Pending,
    ) {
        return false;
    }
    let Some(initial) = snapshot(1, 14) else { return false };
    let Some(ready) = ResourceReadyState::new(
        &port.descriptor,
        initial,
        ResourceChangeCheckpoint::NotRequired,
    )
    .ok() else {
        return false;
    };
    if !port.complete_open(ResourceOpenOutcome::ready(ready)) {
        return false;
    }
    let delivery = ResourceAccessPort::take_open_completion(port, operation);
    let Some(completion) = delivery.into_completion() else {
        return false;
    };
    completion.operation() == operation
        && completion.request().resource().value() == 11
        && completion.outcome().status() == ResourceOpenStatus::Ready
        && completion
            .outcome()
            .ready_state()
            .is_some_and(|state| state.snapshot().same_snapshot(initial))
        && completion_is(
            ResourceAccessPort::take_open_completion(port, operation).status(),
            ResourceCompletionUnavailable::AlreadyDelivered,
        )
}

fn observation_journey(port: &mut ScriptedResourcePort) -> bool {
    let Some(request) = observation_request(&port.descriptor, 20) else {
        return false;
    };
    let operation = request.operation();
    let admission = ResourceAccessPort::submit_observation(port, request);
    if admission.status() != ResourceAdmissionStatus::Admitted
        || admission.admitted_operation() != Some(operation)
    {
        return false;
    }
    if !pressure_returns_request(port) {
        return false;
    }
    let Some(outcome) = observed_outcome(&port.descriptor) else {
        return false;
    };
    if !port.complete_observation(outcome) {
        return false;
    }
    let delivery =
        ResourceAccessPort::take_observation_completion(port, operation);
    let Some(completion) = delivery.into_completion() else {
        return false;
    };
    observation_completion_matches(completion, operation)
}

fn pressure_returns_request(port: &mut ScriptedResourcePort) -> bool {
    let Some(request) = observation_request(&port.descriptor, 21) else {
        return false;
    };
    let admission = ResourceAccessPort::submit_observation(port, request);
    let returned_bytes = admission
        .returned_request()
        .is_some_and(|value| value.regions().bytes() == [0, 1, 1]);
    admission.status() == ResourceAdmissionStatus::NotAdmitted
        && admission.failure() == Some(ResourceAdmissionFailure::Pressure)
        && returned_bytes
        && admission
            .into_returned_request()
            .is_some_and(|value| value.into_regions().bytes() == [0, 1, 1])
}

fn observed_outcome(
    descriptor: &ResourceAccessDescriptor,
) -> Option<ResourceObservationOutcome> {
    let stamp = descriptor.carriers().payload().selected_stamp();
    let prepared = descriptor.prepare_payload_carrier(stamp, 1, 5).ok()?;
    let payload = descriptor
        .bind_payload_carrier(prepared, b"hello".as_slice().into())
        .ok()?;
    let observed = ResourceObservedState::new(
        descriptor,
        ResourceObservationTarget::Current(snapshot(1, 14)?.access()),
        CheckedCount::new(1, ZeroPolicy::Reject).ok()?,
        snapshot(1, 14)?,
        payload,
    )
    .ok()?;
    Some(ResourceObservationOutcome::observed(observed))
}

fn observation_completion_matches(
    completion: ResourceObservationCompletion,
    operation: ResourceOperationStamp,
) -> bool {
    if completion.operation() != operation
        || completion.request().regions().bytes() != [0, 1, 1]
    {
        return false;
    }
    let (_, outcome) = completion.into_parts();
    let Some(observed) = outcome.into_observed_state() else {
        return false;
    };
    let Some(expected) = snapshot(1, 14) else { return false };
    observed.snapshot().same_snapshot(expected)
        && observed.payload().bytes() == b"hello"
        && observed.into_payload().into_bytes().as_ref() == b"hello"
}

fn cancellation_and_close_journey(port: &mut ScriptedResourcePort) -> bool {
    let Some(request) = observation_request(&port.descriptor, 30) else {
        return false;
    };
    let operation = request.operation();
    if ResourceAccessPort::submit_observation(port, request).status()
        != ResourceAdmissionStatus::Admitted
    {
        return false;
    }
    let Some(cancellation) = ResourceCancellationRequest::new(
        operation,
        CancellationCause::DeadlineReached,
    )
    .ok() else {
        return false;
    };
    let first = ResourceAccessPort::request_cancellation(port, cancellation);
    let second = ResourceAccessPort::request_cancellation(port, cancellation);
    if !cancellation_requested(first) || !cancellation_repeated(second) {
        return false;
    }
    let Some(close) = close_request(&port.descriptor, 31) else {
        return false;
    };
    if !close_is_incomplete(ResourceAccessPort::close(port, close)) {
        return false;
    }
    if !port.complete_observation(ResourceObservationOutcome::not_observed(
        ResourceObservationFailure::Cancelled,
    )) {
        return false;
    }
    let delivery =
        ResourceAccessPort::take_observation_completion(port, operation);
    if !cancelled_completion(delivery) {
        return false;
    }
    let Some(close) = close_request(&port.descriptor, 32) else {
        return false;
    };
    ResourceAccessPort::close(port, close) == ResourceCloseOutcome::Closed
        && ResourceAccessPort::close(port, close)
            == ResourceCloseOutcome::Closed
}

fn cancellation_requested(result: ResourceCancellationResult) -> bool {
    match result {
        ResourceCancellationResult::Requested(outcome) => {
            outcome.cause() == CancellationCause::DeadlineReached
                && outcome.execution() == ExecutionOutcome::OwnedNonterminal
        }
        _ => false,
    }
}

fn cancellation_repeated(result: ResourceCancellationResult) -> bool {
    match result {
        ResourceCancellationResult::AlreadyRequested(outcome) => {
            outcome.cause() == CancellationCause::DeadlineReached
                && outcome.execution() == ExecutionOutcome::OwnedNonterminal
        }
        _ => false,
    }
}

fn close_is_incomplete(outcome: ResourceCloseOutcome) -> bool {
    match outcome {
        ResourceCloseOutcome::Incomplete(retained) => {
            retained.execution() == ExecutionOutcome::OwnedNonterminal
                && retained.unresolved().value() == 1
        }
        ResourceCloseOutcome::Closed | ResourceCloseOutcome::StaleAdapter => {
            false
        }
    }
}

fn cancelled_completion(
    delivery: ResourceObservationCompletionDelivery,
) -> bool {
    let Some(completion) = delivery.into_completion() else {
        return false;
    };
    let (_, outcome) = completion.into_parts();
    outcome.failure() == Some(ResourceObservationFailure::Cancelled)
        && outcome.observed_state().is_none()
}

fn open_request(
    descriptor: &ResourceAccessDescriptor,
    request: u128,
    resource: u128,
) -> Option<ResourceOpenRequest> {
    ResourceOpenRequest::new(
        descriptor,
        operation(descriptor, ResourceOperationKind::Open, request),
        ResourceIdentity::new(resource),
    )
    .ok()
}

fn observation_request(
    descriptor: &ResourceAccessDescriptor,
    request: u128,
) -> Option<ResourceObservationRequest> {
    let stamp = descriptor.carriers().regions().selected_stamp();
    let prepared = descriptor.prepare_region_carrier(stamp, 1, 3).ok()?;
    let regions = descriptor
        .bind_region_carrier(prepared, [0_u8, 1, 1].as_slice().into())
        .ok()?;
    ResourceObservationRequest::new(
        descriptor,
        operation(descriptor, ResourceOperationKind::Observation, request),
        ResourceObservationTarget::Current(snapshot(1, 14)?.access()),
        regions,
    )
    .ok()
}

fn close_request(
    descriptor: &ResourceAccessDescriptor,
    request: u128,
) -> Option<ResourceCloseRequest> {
    ResourceCloseRequest::new(
        descriptor,
        operation(descriptor, ResourceOperationKind::Close, request),
    )
    .ok()
}

const fn operation(
    descriptor: &ResourceAccessDescriptor,
    kind: ResourceOperationKind,
    request: u128,
) -> ResourceOperationStamp {
    ResourceOperationStamp::new(
        descriptor.adapter(),
        kind,
        ResourceRequestIdentity::new(request),
        ResourceCancellationIdentity::new(request),
        ResourceCorrelationIdentity::new(500),
        ResourceDeadline::Governed(ResourceDeadlineIdentity::new(600)),
    )
}

const fn stale_operation(
    descriptor: &ResourceAccessDescriptor,
    kind: ResourceOperationKind,
    request: u128,
) -> ResourceOperationStamp {
    let adapter = descriptor.adapter();
    let stale = LifecycleIdentity::new(
        adapter.instance(),
        adapter.incarnation(),
        Generation::new(2),
    );
    ResourceOperationStamp::new(
        stale,
        kind,
        ResourceRequestIdentity::new(request),
        ResourceCancellationIdentity::new(request),
        ResourceCorrelationIdentity::new(500),
        ResourceDeadline::None,
    )
}

fn completion_is(
    status: ResourceCompletionStatus,
    expected: ResourceCompletionUnavailable,
) -> bool {
    status == ResourceCompletionStatus::Unavailable(expected)
}

const fn open_unavailable(
    cause: ResourceCompletionUnavailable,
) -> ResourceOpenCompletionDelivery {
    ResourceOpenCompletionDelivery::unavailable(cause)
}

const fn observation_unavailable(
    cause: ResourceCompletionUnavailable,
) -> ResourceObservationCompletionDelivery {
    ResourceObservationCompletionDelivery::unavailable(cause)
}

fn pending_operation(
    request: Option<&ResourceOpenRequest>,
) -> Option<ResourceOperationStamp> {
    request.map(|value| value.operation())
}

fn pending_observation(
    request: Option<&ResourceObservationRequest>,
) -> Option<ResourceOperationStamp> {
    request.map(ResourceObservationRequest::operation)
}

fn completion_operation(
    completion: Option<&ResourceOpenCompletion>,
) -> Option<ResourceOperationStamp> {
    completion.map(ResourceOpenCompletion::operation)
}

fn observation_completion_operation(
    completion: Option<&ResourceObservationCompletion>,
) -> Option<ResourceOperationStamp> {
    completion.map(ResourceObservationCompletion::operation)
}
