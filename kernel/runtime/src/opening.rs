use reovim_kernel_core::Buffer;
use reovim_kernel_interface::cancellation::{
    CancellationCause, CancellationState, DeadlineState, InstallationOutcome,
};
use reovim_kernel_interface::event::editing::{EditingContext, EditingStage};
use reovim_kernel_interface::interaction::root::{
    RootFailure, RootOpenFailure, RootOpenRequest, RootOpenTarget,
    RootOpenedBuffer,
};
use reovim_kernel_interface::lifecycle::RollbackOutcome;
use reovim_kernel_interface::resource::access::ResourceCloseOutcome;
use reovim_kernel_interface::resource::delivery::{
    ResourceCompletionStatus, ResourceCompletionUnavailable,
};
use reovim_kernel_interface::resource::operation::{
    ResourceCancellationRequest, ResourceOpenRequest, ResourceOperationStamp,
};
use reovim_kernel_interface::resource::port::ResourceAccessPort;
use reovim_kernel_interface::resource::state::{
    ResourceStateInput, ResourceStateModel, ResourceStatePreparation,
};
use reovim_kernel_interface::semantic::domain::DomainPolicyPort;
use reovim_kernel_interface::semantic::representation::RepresentationPolicyPort;
use reovim_kernel_interface::semantic::snapshot::RepresentationSnapshotPort;

use crate::{Runtime, RuntimeError};

pub struct OpenRecord {
    pub request: RootOpenRequest,
    pub outcome: Option<Result<RootOpenedBuffer, RootOpenFailure>>,
    pub delivered: bool,
    pub cancellation: Option<CancellationCause>,
}

#[derive(Clone, Copy)]
pub enum Opening {
    Open { record: usize, operation: ResourceOperationStamp },
    Observation { record: usize, operation: ResourceOperationStamp },
}

impl<A, M, D, R, S> Runtime<A, M, D, R, S>
where
    A: ResourceAccessPort,
    M: ResourceStateModel,
    D: DomainPolicyPort<State = M::State>,
    R: RepresentationPolicyPort<State = M::State>,
    S: RepresentationSnapshotPort<State = M::State>,
{
    pub(crate) fn progress_open(&mut self) -> Result<bool, RuntimeError> {
        self.forward_open_cancellation();
        match self.opening {
            Some(Opening::Open { record, operation }) => {
                self.poll_resource_open(record, operation)
            }
            Some(Opening::Observation { record, operation }) => {
                self.poll_observation(record, operation)
            }
            None => self.begin_open(),
        }
    }

    fn forward_open_cancellation(&mut self) {
        let Some(opening) = self.opening else {
            return;
        };
        let (Opening::Open { record, operation }
        | Opening::Observation { record, operation }) = opening;
        let Some(cause) = self.opens[record].cancellation else {
            return;
        };
        let request = ResourceCancellationRequest::new(operation, cause)
            .expect("open/observation are cancellable operation kinds");
        let _ = self.resource.request_cancellation(request);
    }

    fn begin_open(&mut self) -> Result<bool, RuntimeError> {
        let Some(index) = self.opens.iter().position(|r| r.outcome.is_none())
        else {
            return Ok(false);
        };
        if self.opens[index].cancellation.is_some() {
            self.fail_open(index, RootFailure::Cancelled, false);
            return Ok(true);
        }
        if !self.providers_current() {
            self.fail_open(index, RootFailure::Stale, false);
            return Ok(true);
        }
        match self.opens[index].request.target {
            RootOpenTarget::Existing(_) => self.publish_open(index)?,
            RootOpenTarget::Independent(request) => {
                self.submit_resource_open(index, request);
            }
        }
        Ok(true)
    }

    fn submit_resource_open(
        &mut self,
        index: usize,
        request: ResourceOpenRequest,
    ) {
        let admission = self.resource.submit_open(request);
        if admission.admitted_operation() != Some(request.operation()) {
            self.fail_open(index, RootFailure::ProviderFailure, false);
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
    ) -> Result<bool, RuntimeError> {
        let delivery = self.resource.take_open_completion(operation);
        let status = delivery.status();
        let Some(completion) = delivery.into_completion() else {
            return Ok(self.missing_open(index, status));
        };
        let expected = self.config.buffer.source;
        let ready = completion.outcome().ready_state();
        if completion.operation() != operation
            || self.opens[index].request.target
                != RootOpenTarget::Independent(completion.request())
            || ready.is_none_or(|ready| ready.snapshot() != expected)
            || self.opens[index].cancellation.is_some()
        {
            let cause = cancellation_or_provider(
                self.opens[index].cancellation.is_some(),
            );
            self.fail_open(index, cause, true);
            return Ok(true);
        }
        let observation =
            self.observation.take().ok_or(RuntimeError::Provider)?;
        let operation = observation.operation();
        let admission = self.resource.submit_observation(observation);
        if admission.admitted_operation() != Some(operation) {
            self.observation = admission.into_returned_request();
            self.fail_open(index, RootFailure::ProviderFailure, true);
            return Ok(true);
        }
        self.opening = Some(Opening::Observation { record: index, operation });
        Ok(true)
    }

    fn poll_observation(
        &mut self,
        index: usize,
        operation: ResourceOperationStamp,
    ) -> Result<bool, RuntimeError> {
        let delivery = self.resource.take_observation_completion(operation);
        let status = delivery.status();
        let Some(completion) = delivery.into_completion() else {
            return Ok(self.missing_open(index, status));
        };
        if completion.operation() != operation
            || self.opens[index].cancellation.is_some()
        {
            let cause = cancellation_or_provider(
                self.opens[index].cancellation.is_some(),
            );
            self.fail_open(index, cause, true);
            return Ok(true);
        }
        let (_, outcome) = completion.into_parts();
        let Some(observed) = outcome.into_observed_state() else {
            self.fail_open(index, RootFailure::ProviderFailure, true);
            return Ok(true);
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
            return Ok(true);
        };
        self.buffer = Some(buffer);
        self.opening = None;
        self.publish_open(index)?;
        Ok(true)
    }

    fn missing_open(
        &mut self,
        index: usize,
        status: ResourceCompletionStatus,
    ) -> bool {
        if status
            == ResourceCompletionStatus::Unavailable(
                ResourceCompletionUnavailable::Pending,
            )
        {
            return false;
        }
        self.fail_open(index, RootFailure::ProviderFailure, true);
        true
    }

    fn publish_open(&mut self, index: usize) -> Result<(), RuntimeError> {
        let state = self.buffer.as_ref().ok_or(RuntimeError::Provider)?.stamp();
        let client = self.opens[index].request.client;
        let session = self
            .sessions
            .iter()
            .position(|s| s.client == Some(client))
            .ok_or(RuntimeError::Provider)?;
        self.sessions[session].current = Some(state);
        if self.initial_snapshot(session).is_err() {
            self.sessions[session].current = None;
            let independent = matches!(
                self.opens[index].request.target,
                RootOpenTarget::Independent(_)
            );
            self.fail_open(index, RootFailure::ProviderFailure, independent);
            return Ok(());
        }
        self.sessions[session].phase = crate::session::Phase::Opened;
        self.opens[index].outcome = Some(Ok(RootOpenedBuffer {
            state,
            editable: true,
            history: false,
        }));
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
        Ok(())
    }

    pub(crate) fn fail_open(
        &mut self,
        index: usize,
        cause: RootFailure,
        close: bool,
    ) {
        if close && !self.sessions.iter().any(crate::Session::ready) {
            self.buffer = None;
        }
        let rollback = if !close
            || self.resource.close(self.resource_close)
                == ResourceCloseOutcome::Closed
        {
            RollbackOutcome::Reverted
        } else {
            RollbackOutcome::IncompleteShutdown
        };
        self.opens[index].outcome =
            Some(Err(RootOpenFailure { cause, rollback }));
        self.opening = None;
    }
}

const fn cancellation_or_provider(cancelled: bool) -> RootFailure {
    if cancelled {
        RootFailure::Cancelled
    } else {
        RootFailure::ProviderFailure
    }
}
