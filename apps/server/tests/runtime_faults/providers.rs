use std::{cell::Cell, rc::Rc};

use reovim_kernel_interface::operation::ContractSelection;
use reovim_kernel_interface::resource::access::{
    ResourceAccessDescriptor, ResourceApplicabilityOutcome,
    ResourceCloseOutcome,
};
use reovim_kernel_interface::resource::delivery::{
    ResourceCancellationResult, ResourceCompletionUnavailable,
    ResourceObservationAdmission, ResourceObservationCompletionDelivery,
    ResourceOpenAdmission, ResourceOpenCompletionDelivery,
};
use reovim_kernel_interface::resource::operation::{
    ResourceCancellationRequest, ResourceCloseRequest,
    ResourceObservationRequest, ResourceOpenRequest, ResourceOperationStamp,
};
use reovim_kernel_interface::resource::port::ResourceAccessPort;
use reovim_kernel_interface::semantic::policy::SemanticPolicyFailure;
use reovim_kernel_interface::semantic::representation::RepresentationPolicyPort;
use reovim_kernel_interface::semantic::snapshot::{
    RepresentationSnapshotDescriptor, RepresentationSnapshotPayload,
    RepresentationSnapshotPort, RepresentationSnapshotRequest,
};
use reovim_representation_basic::{HeadingRepresentation, HeadingSnapshot};
use reovim_resource_local::{ByteState, MemoryResource};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapshotMode {
    Good,
    Failure,
    OtherSchema,
}

pub struct FaultSnapshot {
    inner: HeadingSnapshot,
    alternate: RepresentationSnapshotDescriptor,
    mode: Rc<Cell<SnapshotMode>>,
}

impl FaultSnapshot {
    pub(super) fn new(
        inner: HeadingSnapshot,
        policy: &HeadingRepresentation,
        mode: Rc<Cell<SnapshotMode>>,
    ) -> Self {
        let selected = inner.descriptor();
        let alternate = RepresentationSnapshotDescriptor::new(
            policy.descriptor(),
            ContractSelection::new(0xdead, 1, 1, 1).expect("alternate schema"),
            selected.call(),
            selected.limits(),
        );
        Self { inner, alternate, mode }
    }
}

impl RepresentationSnapshotPort for FaultSnapshot {
    type State = ByteState;

    fn descriptor(&self) -> &RepresentationSnapshotDescriptor {
        if self.mode.get() == SnapshotMode::OtherSchema {
            &self.alternate
        } else {
            self.inner.descriptor()
        }
    }

    fn snapshot(
        &self,
        request: &RepresentationSnapshotRequest<'_, ByteState>,
    ) -> Result<RepresentationSnapshotPayload, SemanticPolicyFailure> {
        if self.mode.get() == SnapshotMode::Failure {
            return Err(SemanticPolicyFailure::ProviderFailure);
        }
        let mut payload = self.inner.snapshot(request)?;
        if self.mode.get() == SnapshotMode::OtherSchema {
            payload.schema = self.alternate.output_schema();
        }
        Ok(payload)
    }
}

pub struct DeferredOpen {
    inner: MemoryResource,
    cancellation_seen: Rc<Cell<bool>>,
    admitted: Option<ResourceOperationStamp>,
    released: bool,
}

impl DeferredOpen {
    pub(super) const fn new(
        inner: MemoryResource,
        cancellation_seen: Rc<Cell<bool>>,
    ) -> Self {
        Self { inner, cancellation_seen, admitted: None, released: false }
    }
}

impl ResourceAccessPort for DeferredOpen {
    fn descriptor(&self) -> &ResourceAccessDescriptor {
        self.inner.descriptor()
    }

    fn applicability(
        &self,
        request: &ResourceOpenRequest,
    ) -> ResourceApplicabilityOutcome {
        self.inner.applicability(request)
    }

    fn submit_open(
        &mut self,
        request: ResourceOpenRequest,
    ) -> ResourceOpenAdmission {
        let admission = self.inner.submit_open(request);
        if admission.admitted_operation() == Some(request.operation()) {
            self.admitted = Some(request.operation());
        }
        admission
    }

    fn submit_observation(
        &mut self,
        request: ResourceObservationRequest,
    ) -> ResourceObservationAdmission {
        self.inner.submit_observation(request)
    }

    fn request_cancellation(
        &mut self,
        request: ResourceCancellationRequest,
    ) -> ResourceCancellationResult {
        if self.admitted == Some(request.operation()) {
            self.cancellation_seen.set(true);
            self.released = true;
        }
        self.inner.request_cancellation(request)
    }

    fn take_open_completion(
        &mut self,
        operation: ResourceOperationStamp,
    ) -> ResourceOpenCompletionDelivery {
        if self.admitted == Some(operation) && !self.released {
            return ResourceOpenCompletionDelivery::unavailable(
                ResourceCompletionUnavailable::Pending,
            );
        }
        self.inner.take_open_completion(operation)
    }

    fn take_observation_completion(
        &mut self,
        operation: ResourceOperationStamp,
    ) -> ResourceObservationCompletionDelivery {
        self.inner.take_observation_completion(operation)
    }

    fn close(&mut self, request: ResourceCloseRequest) -> ResourceCloseOutcome {
        self.inner.close(request)
    }
}
