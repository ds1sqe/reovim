use reovim_kernel_core::Buffer;
use reovim_kernel_interface::cancellation::{
    CancellationCause, CancellationState, DeadlineState, InstallationOutcome,
};
use reovim_kernel_interface::event::editing::{EditingContext, EditingStage};
use reovim_kernel_interface::interaction::root::{
    RootFailure, RootOpenFailure, RootOpenRequest, RootOpenTarget,
    RootOpenedBuffer,
};
use reovim_kernel_interface::lifecycle::{LifecycleState, RollbackOutcome};
use reovim_kernel_interface::resource::access::{
    ResourceCloseOutcome, ResourceOpenFailure,
};
use reovim_kernel_interface::resource::delivery::{
    ResourceCompletionStatus, ResourceCompletionUnavailable,
};
use reovim_kernel_interface::resource::operation::{
    ResourceCancellationRequest, ResourceOpenRequest, ResourceOperationKind,
    ResourceOperationStamp,
};
use reovim_kernel_interface::resource::port::ResourceAccessPort;
use reovim_kernel_interface::resource::state::{
    ResourceStateInput, ResourceStateModel, ResourceStatePreparation,
};
use reovim_kernel_interface::semantic::domain::DomainPolicyPort;
use reovim_kernel_interface::semantic::representation::RepresentationPolicyPort;
use reovim_kernel_interface::semantic::snapshot::RepresentationSnapshotPort;

use crate::Runtime;

pub struct OpenRecord {
    pub request: RootOpenRequest,
    pub outcome: Option<Result<RootOpenedBuffer, RootOpenFailure>>,
    pub delivered: bool,
    pub cancellation: Option<CancellationCause>,
}

#[derive(Clone, Copy)]
pub enum Opening {
    Open {
        record: usize,
        operation: ResourceOperationStamp,
    },
    Observation {
        record: usize,
        operation: ResourceOperationStamp,
    },
    /// A failed Root result does not release unfinished Resource ownership.
    Rollback {
        record: usize,
        operation: Option<ResourceOperationStamp>,
    },
}

impl Opening {
    pub const fn record(self) -> usize {
        match self {
            Self::Open { record, .. }
            | Self::Observation { record, .. }
            | Self::Rollback { record, .. } => record,
        }
    }

    const fn operation(self) -> Option<ResourceOperationStamp> {
        match self {
            Self::Open { operation, .. }
            | Self::Observation { operation, .. } => Some(operation),
            Self::Rollback { operation, .. } => operation,
        }
    }
}

impl<A, M, D, R, S> Runtime<A, M, D, R, S>
where
    A: ResourceAccessPort,
    M: ResourceStateModel,
    D: DomainPolicyPort<State = M::State>,
    R: RepresentationPolicyPort<State = M::State>,
    S: RepresentationSnapshotPort<State = M::State>,
{
    pub(crate) fn progress_open(&mut self) -> bool {
        self.forward_open_cancellation();
        match self.opening {
            Some(Opening::Open { record, operation }) => {
                self.poll_resource_open(record, operation)
            }
            Some(Opening::Observation { record, operation }) => {
                self.poll_observation(record, operation)
            }
            Some(Opening::Rollback { record, operation }) => {
                self.progress_rollback(record, operation)
            }
            None => self.begin_open(),
        }
    }

    fn forward_open_cancellation(&mut self) {
        let Some(opening) = self.opening else {
            return;
        };
        let record = opening.record();
        let Some(operation) = opening.operation() else { return };
        let Some(cause) = self.opens[record].cancellation else {
            return;
        };
        let request = ResourceCancellationRequest::new(operation, cause)
            .expect("open/observation are cancellable operation kinds");
        let _ = self.resource.request_cancellation(request);
    }

    fn begin_open(&mut self) -> bool {
        let Some(index) = self.opens.iter().position(|r| r.outcome.is_none())
        else {
            return false;
        };
        if self.opens[index].cancellation.is_some() {
            self.fail_open(index, RootFailure::Cancelled, false);
            return true;
        }
        if !self.providers_current() {
            self.fail_open(index, RootFailure::Stale, false);
            return true;
        }
        match self.opens[index].request.target {
            RootOpenTarget::Existing(_) => self.publish_open(index),
            RootOpenTarget::Independent(request) => {
                self.submit_resource_open(index, request);
            }
        }
        true
    }

    fn submit_resource_open(
        &mut self,
        index: usize,
        request: ResourceOpenRequest,
    ) {
        let admission = self.resource.submit_open(request);
        if admission.admitted_operation() != Some(request.operation()) {
            let retained = admission.admitted_operation().is_some();
            self.opening = retained.then_some(Opening::Open {
                record: index,
                operation: request.operation(),
            });
            self.fail_open(index, RootFailure::ProviderFailure, retained);
            return;
        }
        self.opening = Some(Opening::Open {
            record: index,
            operation: request.operation(),
        });
    }

    fn poll_resource_open(
        &mut self,
        index: usize,
        operation: ResourceOperationStamp,
    ) -> bool {
        let delivery = self.resource.take_open_completion(operation);
        let status = delivery.status();
        let Some(completion) = delivery.into_completion() else {
            return self.missing_open(index, status);
        };
        let expected = self.config.buffer.source;
        if completion.operation() != operation
            || self.opens[index].request.target
                != RootOpenTarget::Independent(completion.request())
        {
            self.fail_open(index, RootFailure::ProviderFailure, true);
            return true;
        }
        // Only this exact terminal completion releases the pending operation.
        self.opening = None;
        let ready = completion.outcome().ready_state();
        if self.opens[index].cancellation.is_some() {
            self.fail_open(index, RootFailure::Cancelled, true);
            return true;
        }
        if let Some(cause) = completion.outcome().failure() {
            self.fail_open(index, open_failure(cause), true);
            return true;
        }
        if ready.is_none_or(|ready| ready.snapshot() != expected) {
            let cause = cancellation_or_provider(
                self.opens[index].cancellation.is_some(),
            );
            self.fail_open(index, cause, true);
            return true;
        }
        let Some(observation) = self.observation.take() else {
            self.fail_open(index, RootFailure::ProviderFailure, true);
            return true;
        };
        let operation = observation.operation();
        self.opening = Some(Opening::Observation { record: index, operation });
        let admission = self.resource.submit_observation(observation);
        if admission.admitted_operation() != Some(operation) {
            self.opening = admission
                .admitted_operation()
                .map(|_| Opening::Observation { record: index, operation });
            self.observation = admission.into_returned_request();
            self.fail_open(index, RootFailure::ProviderFailure, true);
            return true;
        }
        self.opening = Some(Opening::Observation { record: index, operation });
        true
    }

    fn poll_observation(
        &mut self,
        index: usize,
        operation: ResourceOperationStamp,
    ) -> bool {
        let delivery = self.resource.take_observation_completion(operation);
        let status = delivery.status();
        let Some(completion) = delivery.into_completion() else {
            return self.missing_open(index, status);
        };
        if completion.operation() != operation {
            self.fail_open(index, RootFailure::ProviderFailure, true);
            return true;
        }
        self.opening = None;
        if self.opens[index].cancellation.is_some() {
            let cause = cancellation_or_provider(
                self.opens[index].cancellation.is_some(),
            );
            self.fail_open(index, cause, true);
            return true;
        }
        let (_, outcome) = completion.into_parts();
        let Some(observed) = outcome.into_observed_state() else {
            self.fail_open(index, RootFailure::ProviderFailure, true);
            return true;
        };
        let initialized = ResourceStatePreparation::new(
            self.model.binding(),
            ResourceStateInput::Initial(&observed),
            self.config.buffer.preparation,
            CancellationState::Open,
            DeadlineState::Pending,
        )
        .ok()
        .and_then(|preparation| {
            let expected = preparation.stamp();
            let candidate = preparation.prepare_with(&self.model).ok()?;
            Buffer::new(
                &self.config.buffer,
                &expected,
                candidate,
                InstallationOutcome::Current,
            )
            .ok()
        });
        let Some(buffer) = initialized.filter(|_| self.providers_current())
        else {
            self.fail_open(index, RootFailure::ProviderFailure, true);
            return true;
        };
        self.buffer = Some(buffer);
        self.opening = None;
        self.publish_open(index);
        true
    }

    fn missing_open(
        &mut self,
        index: usize,
        status: ResourceCompletionStatus,
    ) -> bool {
        let pending = status
            == ResourceCompletionStatus::Unavailable(
                ResourceCompletionUnavailable::Pending,
            );
        if pending && self.opens[index].cancellation.is_none() {
            return false;
        }
        let cause =
            cancellation_or_provider(self.opens[index].cancellation.is_some());
        self.fail_open(index, cause, true);
        true
    }

    fn publish_open(&mut self, index: usize) {
        let independent = matches!(
            self.opens[index].request.target,
            RootOpenTarget::Independent(_)
        );
        let Some(state) = self.buffer.as_ref().map(Buffer::stamp) else {
            self.fail_open(index, RootFailure::ProviderFailure, independent);
            return;
        };
        let client = self.opens[index].request.client;
        let Some(session) =
            self.sessions.iter().position(|s| s.client == Some(client))
        else {
            self.fail_open(index, RootFailure::Stale, independent);
            return;
        };
        if let Some(cause) = self.publication_failure(index, session) {
            self.reject_publication(index, session, cause);
            return;
        }
        self.sessions[session].current = Some(state);
        if self.initial_snapshot(session).is_err() {
            self.reject_publication(
                index,
                session,
                RootFailure::ProviderFailure,
            );
            return;
        }
        if let Some(cause) = self.publication_failure(index, session) {
            self.reject_publication(index, session, cause);
            return;
        }
        self.sessions[session].phase = crate::session::Phase::Opened;
        self.opens[index].outcome =
            Some(Ok(RootOpenedBuffer { state, editable: true, history: true }));
        let _ = self.telemetry.emit(
            EditingStage::Opened,
            &EditingContext {
                session: Some(
                    self.sessions[session].binding.session.stamp().handle(),
                ),
                request: Some(self.opens[index].request.request),
                state: Some(state),
                ..EditingContext::default()
            },
        );
    }

    pub(crate) fn fail_open(
        &mut self,
        index: usize,
        cause: RootFailure,
        close: bool,
    ) {
        let operation = self
            .opening
            .filter(|opening| opening.record() == index)
            .and_then(Opening::operation);
        if close && !self.sessions.iter().any(crate::Session::ready) {
            self.buffer = None;
        }
        let complete = !close
            || self.resource.close(self.resource_close)
                == ResourceCloseOutcome::Closed;
        let rollback = if complete {
            RollbackOutcome::Reverted
        } else {
            RollbackOutcome::IncompleteShutdown
        };
        self.opens[index].outcome =
            Some(Err(RootOpenFailure { cause, rollback }));
        self.opening = if complete {
            None
        } else {
            self.opens[index]
                .cancellation
                .get_or_insert(CancellationCause::OwnerClosing);
            Some(Opening::Rollback { record: index, operation })
        };
    }

    fn progress_rollback(
        &mut self,
        record: usize,
        operation: Option<ResourceOperationStamp>,
    ) -> bool {
        let drained =
            operation.is_some_and(|operation| self.drain_operation(operation));
        let operation = if drained { None } else { operation };
        if self.resource.close(self.resource_close)
            == ResourceCloseOutcome::Closed
        {
            self.opening = None;
            return true;
        }
        self.opening = Some(Opening::Rollback { record, operation });
        drained
    }

    fn drain_operation(&mut self, operation: ResourceOperationStamp) -> bool {
        match operation.kind() {
            ResourceOperationKind::Open => self
                .resource
                .take_open_completion(operation)
                .into_completion()
                .is_some_and(|completion| completion.operation() == operation),
            ResourceOperationKind::Observation => self
                .resource
                .take_observation_completion(operation)
                .into_completion()
                .is_some_and(|completion| completion.operation() == operation),
            ResourceOperationKind::Close => false,
        }
    }

    fn publication_failure(
        &self,
        record: usize,
        session: usize,
    ) -> Option<RootFailure> {
        if self.opens[record].cancellation.is_some() {
            return Some(RootFailure::Cancelled);
        }
        if self.phase != LifecycleState::Ready
            || self.sessions[session].ending()
        {
            return Some(RootFailure::Closing);
        }
        if !self.providers_current()
            || self.resource.descriptor().adapter()
                != self.resource_close.operation().adapter()
        {
            return Some(RootFailure::Stale);
        }
        None
    }

    fn reject_publication(
        &mut self,
        record: usize,
        session: usize,
        cause: RootFailure,
    ) {
        self.sessions[session].current = None;
        self.sessions[session].initial = None;
        let independent = matches!(
            self.opens[record].request.target,
            RootOpenTarget::Independent(_)
        );
        self.fail_open(record, cause, independent);
    }
}

const fn open_failure(cause: ResourceOpenFailure) -> RootFailure {
    match cause {
        ResourceOpenFailure::Denied => RootFailure::Denied,
        ResourceOpenFailure::Unsupported => RootFailure::Unsupported,
        ResourceOpenFailure::Unavailable => RootFailure::Unavailable,
        ResourceOpenFailure::Stale | ResourceOpenFailure::Replaced => {
            RootFailure::Stale
        }
        ResourceOpenFailure::Pressure => RootFailure::Pressure,
        ResourceOpenFailure::Cancelled => RootFailure::Cancelled,
        _ => RootFailure::ProviderFailure,
    }
}

const fn cancellation_or_provider(cancelled: bool) -> RootFailure {
    if cancelled {
        RootFailure::Cancelled
    } else {
        RootFailure::ProviderFailure
    }
}
