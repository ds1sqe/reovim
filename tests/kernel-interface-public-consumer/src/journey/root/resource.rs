//! One deterministic Resource adapter for the root public-boundary witness.

use reovim_kernel_interface::cancellation::{
    CancellationCause, CancellationOutcome, ExecutionOutcome,
};
use reovim_kernel_interface::lifecycle::RollbackOutcome;
use reovim_kernel_interface::resource::ResourceSnapshotStamp;
use reovim_kernel_interface::resource::access::{
    ResourceAccessDescriptor, ResourceApplicabilityOutcome,
    ResourceChangeCheckpoint, ResourceCloseOutcome, ResourceIncompleteClose,
    ResourceObservationFailure, ResourceObservationOutcome,
    ResourceObservedState, ResourceOpenFailure, ResourceOpenOutcome,
    ResourceReadyState,
};
use reovim_kernel_interface::resource::delivery::{
    ResourceAdmissionFailure, ResourceCancellationResult,
    ResourceCompletionUnavailable, ResourceObservationAdmission,
    ResourceObservationCompletion, ResourceObservationCompletionDelivery,
    ResourceOpenAdmission, ResourceOpenCompletion,
    ResourceOpenCompletionDelivery,
};
use reovim_kernel_interface::resource::operation::{
    ResourceCancellationIdentity, ResourceCancellationRequest,
    ResourceCloseRequest, ResourceCorrelationIdentity, ResourceDeadline,
    ResourceObservationRequest, ResourceObservationTarget, ResourceOpenRequest,
    ResourceOperationKind, ResourceOperationStamp, ResourceRequestIdentity,
};
use reovim_kernel_interface::resource::port::ResourceAccessPort;

use crate::first_edit::native::support::count;

pub struct Resource {
    descriptor: ResourceAccessDescriptor,
    source: ResourceSnapshotStamp,
    observation: Option<ResourceObservedState>,
    pending: Option<ResourceOpenRequest>,
    observation_request: Option<ResourceObservationRequest>,
    delivered: Option<ResourceOperationStamp>,
    deferred: bool,
    cancelled: Option<CancellationCause>,
    failure: Option<ResourceOpenFailure>,
    rollback: RollbackOutcome,
    pub incomplete: bool,
    closed: bool,
}

impl Resource {
    pub fn new(observation: ResourceObservedState) -> Self {
        Self {
            descriptor: crate::resource::descriptor().unwrap(),
            source: observation.snapshot(),
            observation: Some(observation),
            pending: None,
            observation_request: None,
            delivered: None,
            deferred: false,
            cancelled: None,
            failure: None,
            rollback: RollbackOutcome::Reverted,
            incomplete: false,
            closed: false,
        }
    }

    pub const fn fail_next(
        &mut self,
        cause: ResourceOpenFailure,
        rollback: RollbackOutcome,
    ) {
        self.failure = Some(cause);
        self.rollback = rollback;
    }

    pub const fn cancellation_rollback(&mut self, rollback: RollbackOutcome) {
        self.rollback = rollback;
    }

    pub const fn retire_rollback(&mut self) {
        self.incomplete = false;
    }

    pub const fn is_closed(&self) -> bool {
        self.closed
    }

    pub fn open_request(&self, request: u128) -> ResourceOpenRequest {
        ResourceOpenRequest::new(
            &self.descriptor,
            self.operation(ResourceOperationKind::Open, request),
            self.source.access().resource(),
        )
        .unwrap()
    }

    pub fn observe_request(&self, request: u128) -> ResourceObservationRequest {
        let stamp = self.descriptor.carriers().regions().selected_stamp();
        let prepared =
            self.descriptor.prepare_region_carrier(stamp, 1, 3).unwrap();
        let regions = self
            .descriptor
            .bind_region_carrier(prepared, [0_u8, 1, 1].as_slice().into())
            .unwrap();
        ResourceObservationRequest::new(
            &self.descriptor,
            self.operation(ResourceOperationKind::Observation, request),
            ResourceObservationTarget::Snapshot(self.source),
            regions,
        )
        .unwrap()
    }

    pub fn close_request(&self) -> ResourceCloseRequest {
        ResourceCloseRequest::new(
            &self.descriptor,
            self.operation(ResourceOperationKind::Close, 8000),
        )
        .unwrap()
    }

    const fn operation(
        &self,
        kind: ResourceOperationKind,
        request: u128,
    ) -> ResourceOperationStamp {
        ResourceOperationStamp::new(
            self.descriptor.adapter(),
            kind,
            ResourceRequestIdentity::new(request),
            ResourceCancellationIdentity::new(request),
            ResourceCorrelationIdentity::new(8001),
            ResourceDeadline::None,
        )
    }
}

impl ResourceAccessPort for Resource {
    fn descriptor(&self) -> &ResourceAccessDescriptor {
        &self.descriptor
    }

    fn applicability(
        &self,
        request: &ResourceOpenRequest,
    ) -> ResourceApplicabilityOutcome {
        if request.resource() != self.source.access().resource() {
            return ResourceApplicabilityOutcome::Unsupported;
        }
        ResourceApplicabilityOutcome::Applicable
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
        if self.pending.is_some() {
            return ResourceOpenAdmission::not_admitted(
                ResourceAdmissionFailure::Pressure,
                request,
            );
        }
        if request.operation().adapter() != self.descriptor.adapter() {
            return ResourceOpenAdmission::not_admitted(
                ResourceAdmissionFailure::Replaced,
                request,
            );
        }
        if self.applicability(&request)
            != ResourceApplicabilityOutcome::Applicable
        {
            return ResourceOpenAdmission::not_admitted(
                ResourceAdmissionFailure::Unsupported,
                request,
            );
        }
        self.pending = Some(request);
        self.deferred = true;
        ResourceOpenAdmission::admitted(&request)
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
        if self.observation_request.is_some() {
            return ResourceObservationAdmission::not_admitted(
                ResourceAdmissionFailure::Pressure,
                request,
            );
        }
        let admitted = ResourceObservationAdmission::admitted(&request);
        self.observation_request = Some(request);
        admitted
    }

    fn request_cancellation(
        &mut self,
        request: ResourceCancellationRequest,
    ) -> ResourceCancellationResult {
        if self.pending.map(ResourceOpenRequest::operation)
            != Some(request.operation())
        {
            return ResourceCancellationResult::UnknownRequest;
        }
        self.cancelled = Some(request.cause());
        ResourceCancellationResult::Requested(CancellationOutcome::new(
            request.cause(),
            ExecutionOutcome::OwnedNonterminal,
        ))
    }

    fn take_open_completion(
        &mut self,
        operation: ResourceOperationStamp,
    ) -> ResourceOpenCompletionDelivery {
        if self.delivered == Some(operation) {
            return ResourceOpenCompletionDelivery::unavailable(
                ResourceCompletionUnavailable::AlreadyDelivered,
            );
        }
        if self.pending.map(ResourceOpenRequest::operation) != Some(operation) {
            return ResourceOpenCompletionDelivery::unavailable(
                ResourceCompletionUnavailable::UnknownRequest,
            );
        }
        if self.deferred {
            self.deferred = false;
            return ResourceOpenCompletionDelivery::unavailable(
                ResourceCompletionUnavailable::Pending,
            );
        }
        let request = self.pending.take().unwrap();
        self.delivered = Some(operation);
        let failure = self
            .cancelled
            .map(|_| ResourceOpenFailure::Cancelled)
            .or(self.failure);
        let outcome = match failure {
            Some(cause) => {
                self.incomplete =
                    self.rollback == RollbackOutcome::IncompleteShutdown;
                ResourceOpenOutcome::not_ready(cause, self.rollback)
            }
            None => ResourceOpenOutcome::ready(
                ResourceReadyState::new(
                    &self.descriptor,
                    self.source,
                    ResourceChangeCheckpoint::NotRequired,
                )
                .unwrap(),
            ),
        };
        ResourceOpenCompletionDelivery::delivered(ResourceOpenCompletion::new(
            request, outcome,
        ))
    }

    fn take_observation_completion(
        &mut self,
        operation: ResourceOperationStamp,
    ) -> ResourceObservationCompletionDelivery {
        if self
            .observation_request
            .as_ref()
            .map(ResourceObservationRequest::operation)
            != Some(operation)
        {
            return ResourceObservationCompletionDelivery::unavailable(
                ResourceCompletionUnavailable::UnknownRequest,
            );
        }
        let request = self.observation_request.take().unwrap();
        let outcome = self.observation.take().map_or_else(
            || {
                ResourceObservationOutcome::not_observed(
                    ResourceObservationFailure::Unavailable,
                )
            },
            ResourceObservationOutcome::observed,
        );
        ResourceObservationCompletionDelivery::delivered(
            ResourceObservationCompletion::new(request, outcome),
        )
    }

    fn close(&mut self, request: ResourceCloseRequest) -> ResourceCloseOutcome {
        if request.operation().adapter() != self.descriptor.adapter() {
            return ResourceCloseOutcome::StaleAdapter;
        }
        self.closed = true;
        if self.incomplete
            || self.pending.is_some()
            || self.observation_request.is_some()
        {
            return ResourceCloseOutcome::Incomplete(
                ResourceIncompleteClose::new(
                    ExecutionOutcome::OwnedNonterminal,
                    count(1),
                )
                .unwrap(),
            );
        }
        ResourceCloseOutcome::Closed
    }
}
