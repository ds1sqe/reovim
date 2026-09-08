use reovim_kernel_interface::cancellation::ExecutionOutcome;
use reovim_kernel_interface::resource::access::{
    ResourceAccessDescriptor, ResourceApplicabilityOutcome,
    ResourceChangeCheckpoint, ResourceCloseOutcome, ResourceIncompleteClose,
    ResourceObservationFailure, ResourceObservationOutcome,
    ResourceObservedState, ResourceOpenOutcome, ResourceReadyState,
};
use reovim_kernel_interface::resource::delivery::{
    ResourceAdmissionFailure, ResourceCancellationResult,
    ResourceCompletionUnavailable, ResourceObservationAdmission,
    ResourceObservationCompletion, ResourceObservationCompletionDelivery,
    ResourceOpenAdmission, ResourceOpenCompletion,
    ResourceOpenCompletionDelivery,
};
use reovim_kernel_interface::resource::operation::{
    ResourceCancellationRequest, ResourceCloseRequest,
    ResourceObservationRequest, ResourceObservationTarget, ResourceOpenRequest,
    ResourceOperationStamp,
};
use reovim_kernel_interface::resource::port::ResourceAccessPort;

use super::{MemoryResource, State};

impl ResourceAccessPort for MemoryResource {
    fn descriptor(&self) -> &ResourceAccessDescriptor {
        &self.descriptor
    }

    fn applicability(
        &self,
        request: &ResourceOpenRequest,
    ) -> ResourceApplicabilityOutcome {
        if !self
            .descriptor
            .adapter()
            .same_generation(request.operation().adapter())
        {
            return ResourceApplicabilityOutcome::Denied;
        }
        if request.resource() != self.snapshot.access().resource() {
            return ResourceApplicabilityOutcome::Unsupported;
        }
        ResourceApplicabilityOutcome::Applicable
    }

    fn submit_open(
        &mut self,
        request: ResourceOpenRequest,
    ) -> ResourceOpenAdmission {
        if let Some(failure) = self.open_failure(&request) {
            return ResourceOpenAdmission::not_admitted(failure, request);
        }
        let ready = ResourceReadyState::new(
            &self.descriptor,
            self.snapshot,
            ResourceChangeCheckpoint::NotRequired,
        )
        .expect("immutable exact source snapshot");
        self.open_stamp = Some(request.operation());
        self.open = Some(ResourceOpenCompletion::new(
            request,
            ResourceOpenOutcome::ready(ready),
        ));
        self.state = State::Ready;
        ResourceOpenAdmission::admitted(&request)
    }

    fn submit_observation(
        &mut self,
        request: ResourceObservationRequest,
    ) -> ResourceObservationAdmission {
        if let Some(failure) = self.observation_failure(&request) {
            return ResourceObservationAdmission::not_admitted(
                failure, request,
            );
        }
        let admission = ResourceObservationAdmission::admitted(&request);
        let outcome = self.observe(request.target()).map_or_else(
            ResourceObservationOutcome::not_observed,
            ResourceObservationOutcome::observed,
        );
        self.observation_stamp = Some(request.operation());
        self.observation =
            Some(ResourceObservationCompletion::new(request, outcome));
        admission
    }

    fn request_cancellation(
        &mut self,
        request: ResourceCancellationRequest,
    ) -> ResourceCancellationResult {
        let operation = request.operation();
        if !operation.adapter().same_generation(self.descriptor.adapter()) {
            return ResourceCancellationResult::StaleAdapter;
        }
        if self.open_stamp == Some(operation)
            || self.observation_stamp == Some(operation)
        {
            return ResourceCancellationResult::AlreadyTerminal;
        }
        ResourceCancellationResult::UnknownRequest
    }

    fn take_open_completion(
        &mut self,
        operation: ResourceOperationStamp,
    ) -> ResourceOpenCompletionDelivery {
        if self.open_stamp != Some(operation) {
            return ResourceOpenCompletionDelivery::unavailable(
                ResourceCompletionUnavailable::UnknownRequest,
            );
        }
        self.open.take().map_or_else(
            || {
                ResourceOpenCompletionDelivery::unavailable(
                    ResourceCompletionUnavailable::AlreadyDelivered,
                )
            },
            ResourceOpenCompletionDelivery::delivered,
        )
    }

    fn take_observation_completion(
        &mut self,
        operation: ResourceOperationStamp,
    ) -> ResourceObservationCompletionDelivery {
        if self.observation_stamp != Some(operation) {
            return ResourceObservationCompletionDelivery::unavailable(
                ResourceCompletionUnavailable::UnknownRequest,
            );
        }
        self.observation.take().map_or_else(
            || {
                ResourceObservationCompletionDelivery::unavailable(
                    ResourceCompletionUnavailable::AlreadyDelivered,
                )
            },
            ResourceObservationCompletionDelivery::delivered,
        )
    }

    fn close(&mut self, request: ResourceCloseRequest) -> ResourceCloseOutcome {
        if !request
            .operation()
            .adapter()
            .same_generation(self.descriptor.adapter())
        {
            return ResourceCloseOutcome::StaleAdapter;
        }
        self.state = State::Closing;
        if self.open.is_some() || self.observation.is_some() {
            return ResourceCloseOutcome::Incomplete(
                ResourceIncompleteClose::new(
                    ExecutionOutcome::Stopped,
                    crate::profile::count(1),
                )
                .expect("retained terminal delivery"),
            );
        }
        self.source = alloc::vec::Vec::new();
        self.state = State::Closed;
        ResourceCloseOutcome::Closed
    }
}

impl MemoryResource {
    fn open_failure(
        &self,
        request: &ResourceOpenRequest,
    ) -> Option<ResourceAdmissionFailure> {
        if self.applicability(request)
            != ResourceApplicabilityOutcome::Applicable
        {
            return Some(ResourceAdmissionFailure::Denied);
        }
        if let Some(failure) = self.admission_failure() {
            return Some(failure);
        }
        self.open_stamp.map(|_| ResourceAdmissionFailure::DuplicateRequest)
    }

    const fn admission_failure(&self) -> Option<ResourceAdmissionFailure> {
        match self.state {
            State::Closing => Some(ResourceAdmissionFailure::Closing),
            State::Closed => Some(ResourceAdmissionFailure::Closed),
            State::Fresh | State::Ready => None,
        }
    }

    fn observation_failure(
        &self,
        request: &ResourceObservationRequest,
    ) -> Option<ResourceAdmissionFailure> {
        if !request
            .operation()
            .adapter()
            .same_generation(self.descriptor.adapter())
            || !request
                .target()
                .access()
                .same_generation(self.snapshot.access())
        {
            return Some(ResourceAdmissionFailure::StaleAccess);
        }
        if let Some(failure) = self.admission_failure() {
            return Some(failure);
        }
        if self.state != State::Ready || self.open.is_some() {
            return Some(ResourceAdmissionFailure::NotReady);
        }
        if self.observation_stamp.is_some() {
            return Some(ResourceAdmissionFailure::DuplicateRequest);
        }
        if request.regions().stamp()
            != self.descriptor.carriers().regions().selected_stamp()
            || request.regions().bytes() != [0]
            || request.regions().regions().value() != 1
        {
            return Some(ResourceAdmissionFailure::InvalidRequest);
        }
        None
    }

    fn observe(
        &self,
        target: ResourceObservationTarget,
    ) -> Result<ResourceObservedState, ResourceObservationFailure> {
        let bytes = crate::native::copy_bytes(&self.source)
            .map_err(|_| ResourceObservationFailure::Pressure)?;
        let size = u64::try_from(bytes.len())
            .map_err(|_| ResourceObservationFailure::Pressure)?;
        let prepared = self
            .descriptor
            .prepare_payload_carrier(
                self.descriptor.carriers().payload().selected_stamp(),
                1,
                size,
            )
            .map_err(|_| ResourceObservationFailure::Pressure)?;
        let payload = self
            .descriptor
            .bind_payload_carrier(prepared, bytes.into_boxed_slice())
            .map_err(|_| ResourceObservationFailure::AdapterFailure)?;
        ResourceObservedState::new(
            &self.descriptor,
            target,
            prepared.regions(),
            self.snapshot,
            payload,
        )
        .map_err(|_| ResourceObservationFailure::Stale)
    }
}
