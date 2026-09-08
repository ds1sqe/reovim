use reovim_kernel_interface::cancellation::ExecutionOutcome;
use reovim_kernel_interface::lifecycle::RollbackOutcome;
use reovim_kernel_interface::resource::access::{
    ResourceAccessDescriptor, ResourceApplicabilityOutcome,
    ResourceCloseOutcome, ResourceIncompleteClose, ResourceOpenFailure,
    ResourceOpenOutcome,
};
use reovim_kernel_interface::resource::delivery::{
    ResourceCancellationResult, ResourceCompletionStatus,
    ResourceCompletionUnavailable, ResourceObservationAdmission,
    ResourceObservationCompletionDelivery, ResourceOpenAdmission,
    ResourceOpenCompletion, ResourceOpenCompletionDelivery,
};
use reovim_kernel_interface::resource::operation::{
    ResourceCancellationRequest, ResourceCloseRequest,
    ResourceObservationRequest, ResourceOpenRequest, ResourceOperationStamp,
};
use reovim_kernel_interface::resource::port::ResourceAccessPort;
use reovim_kernel_interface::resource::state::{
    ResourceStateError, ResourceStateInput, ResourceStateModel,
    ResourceStateModelBinding, ResourceStateOutput, ResourceStatePreparation,
};
use reovim_kernel_interface::semantic::policy::SemanticPolicyFailure;
use reovim_kernel_interface::semantic::snapshot::{
    RepresentationSnapshotDescriptor, RepresentationSnapshotPayload,
    RepresentationSnapshotPort, RepresentationSnapshotRequest,
};
use reovim_representation_basic::HeadingSnapshot;
use reovim_resource_local::{ByteModel, ByteState, MemoryResource, profile};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    None,
    OpenGap,
    ObservationGap,
    PendingOpen,
    OpenFailure,
    ReplaceAfterInitial,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call {
    Cancelled(ResourceOperationStamp),
    Delivered(ResourceOperationStamp),
    Closing,
    Closed,
}
impl Call {
    pub const fn cancelled(&self) -> Option<ResourceOperationStamp> {
        match self {
            Self::Cancelled(operation) => Some(*operation),
            _ => None,
        }
    }
}
pub struct Control {
    pub fault: Fault,
    pub release: Cell<bool>,
    pub native_fail: Cell<bool>,
    pub snapshot_fail: Cell<bool>,
    pub replaced: Cell<bool>,
    pub operation: Cell<Option<ResourceOperationStamp>>,
    pub calls: RefCell<Vec<Call>>,
}
pub struct FaultResource {
    inner: MemoryResource,
    control: Rc<Control>,
    replacement: ResourceAccessDescriptor,
    gap_seen: bool,
}

impl FaultResource {
    pub fn new(inner: MemoryResource, control: Rc<Control>) -> Self {
        let selected = inner.descriptor();
        let replacement = ResourceAccessDescriptor::new(
            selected.contract(),
            reovim_server::assembly::life(44),
            selected.owner(),
            selected.schemas(),
            selected.capabilities(),
            selected.carriers(),
            selected.bounds(),
        );
        Self { inner, control, replacement, gap_seen: false }
    }

    fn blocked(
        &mut self,
        operation: ResourceOperationStamp,
    ) -> Option<ResourceCompletionUnavailable> {
        if self.control.operation.get() != Some(operation)
            || self.control.release.get()
            || self.control.fault == Fault::OpenFailure
        {
            return None;
        }
        if self.control.fault != Fault::PendingOpen && !self.gap_seen {
            self.gap_seen = true;
            return Some(ResourceCompletionUnavailable::UnknownRequest);
        }
        Some(ResourceCompletionUnavailable::Pending)
    }
}

impl ResourceAccessPort for FaultResource {
    fn descriptor(&self) -> &ResourceAccessDescriptor {
        if self.control.replaced.get() {
            &self.replacement
        } else {
            self.inner.descriptor()
        }
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
        let admitted = self.inner.submit_open(request);
        if matches!(
            self.control.fault,
            Fault::OpenGap | Fault::PendingOpen | Fault::OpenFailure
        ) {
            self.control.operation.set(admitted.admitted_operation());
        }
        admitted
    }
    fn submit_observation(
        &mut self,
        request: ResourceObservationRequest,
    ) -> ResourceObservationAdmission {
        let admitted = self.inner.submit_observation(request);
        if self.control.fault == Fault::ObservationGap {
            self.control.operation.set(admitted.admitted_operation());
        }
        admitted
    }
    fn request_cancellation(
        &mut self,
        request: ResourceCancellationRequest,
    ) -> ResourceCancellationResult {
        self.control
            .calls
            .borrow_mut()
            .push(Call::Cancelled(request.operation()));
        self.inner.request_cancellation(request)
    }
    fn take_open_completion(
        &mut self,
        operation: ResourceOperationStamp,
    ) -> ResourceOpenCompletionDelivery {
        if let Some(cause) = self.blocked(operation) {
            return ResourceOpenCompletionDelivery::unavailable(cause);
        }
        let delivery = self.inner.take_open_completion(operation);
        if delivery.status() != ResourceCompletionStatus::Delivered {
            return delivery;
        }
        self.control.calls.borrow_mut().push(Call::Delivered(operation));
        if self.control.fault != Fault::OpenFailure {
            return delivery;
        }
        let completed = delivery.into_completion().unwrap();
        ResourceOpenCompletionDelivery::delivered(ResourceOpenCompletion::new(
            completed.request(),
            ResourceOpenOutcome::not_ready(
                ResourceOpenFailure::Unavailable,
                RollbackOutcome::IncompleteShutdown,
            ),
        ))
    }
    fn take_observation_completion(
        &mut self,
        operation: ResourceOperationStamp,
    ) -> ResourceObservationCompletionDelivery {
        if let Some(cause) = self.blocked(operation) {
            return ResourceObservationCompletionDelivery::unavailable(cause);
        }
        let delivery = self.inner.take_observation_completion(operation);
        if delivery.status() == ResourceCompletionStatus::Delivered {
            self.control.calls.borrow_mut().push(Call::Delivered(operation));
        }
        delivery
    }
    fn close(&mut self, request: ResourceCloseRequest) -> ResourceCloseOutcome {
        if self.control.replaced.get() {
            return ResourceCloseOutcome::StaleAdapter;
        }
        let released = self.inner.close(request);
        let outcome = if self.control.fault == Fault::OpenFailure
            && !self.control.release.get()
        {
            ResourceCloseOutcome::Incomplete(
                ResourceIncompleteClose::new(
                    ExecutionOutcome::OwnedNonterminal,
                    profile::count(0),
                )
                .unwrap(),
            )
        } else {
            released
        };
        self.control.calls.borrow_mut().push(
            if outcome == ResourceCloseOutcome::Closed {
                Call::Closed
            } else {
                Call::Closing
            },
        );
        outcome
    }
}

pub struct FaultModel {
    pub inner: ByteModel,
    pub control: Rc<Control>,
}
impl ResourceStateModel for FaultModel {
    type State = ByteState;
    fn binding(&self) -> ResourceStateModelBinding {
        self.inner.binding()
    }
    fn plan_descriptor(
        &self,
    ) -> &reovim_kernel_interface::buffer::plan::BufferPlanDescriptor {
        self.inner.plan_descriptor()
    }
    fn prepare(
        &self,
        request: &ResourceStatePreparation<'_, ByteState>,
    ) -> Result<ResourceStateOutput<ByteState>, ResourceStateError> {
        if self.control.native_fail.get()
            && matches!(request.input(), ResourceStateInput::Initial(_))
        {
            return Err(ResourceStateError::ProviderFailure);
        }
        let output = self.inner.prepare(request);
        if self.control.fault == Fault::ReplaceAfterInitial
            && matches!(request.input(), ResourceStateInput::Initial(_))
        {
            self.control.replaced.set(true);
        }
        output
    }
}
pub struct FaultSnapshot {
    pub inner: HeadingSnapshot,
    pub control: Rc<Control>,
}
impl RepresentationSnapshotPort for FaultSnapshot {
    type State = ByteState;
    fn descriptor(&self) -> &RepresentationSnapshotDescriptor {
        self.inner.descriptor()
    }
    fn snapshot(
        &self,
        request: &RepresentationSnapshotRequest<'_, ByteState>,
    ) -> Result<RepresentationSnapshotPayload, SemanticPolicyFailure> {
        if self.control.snapshot_fail.get() {
            return Err(SemanticPolicyFailure::ProviderFailure);
        }
        self.inner.snapshot(request)
    }
}
