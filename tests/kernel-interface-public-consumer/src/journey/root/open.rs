use reovim_kernel_interface::cancellation::{
    CancellationCause, CancellationOutcome, ExecutionOutcome,
};
use reovim_kernel_interface::interaction::root::{
    RootFailure, RootOpenCompletion, RootOpenFailure, RootOpenRequest,
    RootOpenTarget, RootOpenTicket, RootOpenedBuffer,
};
use reovim_kernel_interface::lifecycle::RollbackOutcome;
use reovim_kernel_interface::resource::access::{
    ResourceCloseOutcome, ResourceOpenFailure,
};
use reovim_kernel_interface::resource::delivery::{
    ResourceAdmissionStatus, ResourceCancellationResult,
    ResourceCompletionStatus, ResourceCompletionUnavailable,
};
use reovim_kernel_interface::resource::operation::ResourceCancellationRequest;
use reovim_kernel_interface::resource::operation::ResourceOpenRequest;
use reovim_kernel_interface::resource::port::ResourceAccessPort;

use super::{Open, Root};
use crate::first_edit::Family;
use crate::first_edit::native::support;
use reovim_kernel_interface::cancellation::InstallationOutcome;
use reovim_kernel_interface::resource::state::{
    ResourceStateInput, ResourceStateTarget,
};

impl<Model: Family> Root<Model>
where
    Model::State: Clone,
{
    pub(super) fn admit_open(
        &mut self,
        request: &RootOpenRequest,
    ) -> Result<RootOpenTicket, RootFailure> {
        self.active_client(&request.client)?;
        self.check_work(request.work.value())?;
        let ticket =
            RootOpenTicket { client: request.client, request: request.request };
        if let Some(prior) = self.opens.iter().find(|entry| {
            entry.request.client == request.client
                && entry.request.request == request.request
        }) {
            return (prior.request == *request)
                .then_some(ticket)
                .ok_or(RootFailure::ConflictingRequest);
        }
        if self.opens.len() as u64 >= self.limits.opens.value() {
            return Err(RootFailure::Pressure);
        }
        self.admit_target(request)?;
        self.opens.push(Open {
            request: *request,
            completion: None,
            cancelled: false,
        });
        Ok(ticket)
    }

    fn admit_target(
        &mut self,
        request: &RootOpenRequest,
    ) -> Result<(), RootFailure> {
        match request.target {
            RootOpenTarget::Independent(resource) => {
                self.admit_resource(resource)
            }
            RootOpenTarget::Existing(buffer) => self.check_existing(buffer),
        }
    }

    fn admit_resource(
        &mut self,
        request: ResourceOpenRequest,
    ) -> Result<(), RootFailure> {
        if self.shared.borrow().current.is_some() {
            return Err(RootFailure::Unsupported);
        }
        if self.resource.submit_open(request).status()
            != ResourceAdmissionStatus::Admitted
        {
            return Err(RootFailure::ProviderFailure);
        }
        Ok(())
    }

    fn check_existing(
        &self,
        buffer: reovim_kernel_interface::buffer::BufferIncarnationStamp,
    ) -> Result<(), RootFailure> {
        let shared = self.shared.borrow();
        if buffer != shared.stamp.buffer() {
            return Err(RootFailure::Denied);
        }
        if shared.current.is_none() {
            return Err(RootFailure::NotReady);
        }
        Ok(())
    }

    pub(super) fn finish_open(
        &mut self,
        ticket: RootOpenTicket,
    ) -> Result<Option<RootOpenCompletion>, RootFailure> {
        self.client_index(&ticket.client)?;
        let index = self.open_index(&ticket)?;
        if let Some(completion) = self.opens[index].completion {
            return Ok(Some(completion));
        }
        let request = self.opens[index].request;
        let outcome = match request.target {
            RootOpenTarget::Independent(resource) => {
                self.finish_resource(&request, resource)
            }
            RootOpenTarget::Existing(_) => Some(self.join(&request)),
        };
        Ok(outcome
            .map(|outcome| self.retain_completion(index, ticket, &outcome)))
    }

    fn finish_resource(
        &mut self,
        request: &RootOpenRequest,
        resource: ResourceOpenRequest,
    ) -> Option<Result<RootOpenedBuffer, RootOpenFailure>> {
        let delivery = self.resource.take_open_completion(resource.operation());
        if delivery.status()
            == ResourceCompletionStatus::Unavailable(
                ResourceCompletionUnavailable::Pending,
            )
        {
            return None;
        }
        let Some(completion) = delivery.into_completion() else {
            return Some(Err(self.rollback(RootFailure::ProviderFailure)));
        };
        let (_, outcome) = completion.into_parts();
        if let Some(cause) = outcome.failure() {
            return Some(Err(RootOpenFailure {
                cause: resource_failure(cause),
                rollback: outcome.rollback().unwrap(),
            }));
        }
        Some(
            self.initialize(request, outcome.ready_state().unwrap().snapshot()),
        )
    }

    fn join(
        &mut self,
        request: &RootOpenRequest,
    ) -> Result<RootOpenedBuffer, RootOpenFailure> {
        let index = self.publication_client(request).map_err(failure)?;
        let RootOpenTarget::Existing(buffer) = request.target else {
            return Err(failure(RootFailure::InvalidRequest));
        };
        self.check_existing(buffer).map_err(failure)?;
        self.clients[index].opened = true;
        Ok(RootOpenedBuffer {
            state: self.shared.borrow().stamp,
            editable: true,
            history: true,
        })
    }

    fn initialize(
        &mut self,
        request: &RootOpenRequest,
        ready: reovim_kernel_interface::resource::ResourceSnapshotStamp,
    ) -> Result<RootOpenedBuffer, RootOpenFailure> {
        if let Err(cause) = self.publication_client(request) {
            return Err(self.rollback(cause));
        }
        let observation =
            self.resource.observe_request(request.request.value());
        let operation = observation.operation();
        if self.resource.submit_observation(observation).status()
            != ResourceAdmissionStatus::Admitted
        {
            return Err(self.rollback(RootFailure::ProviderFailure));
        }
        let completion = self
            .resource
            .take_observation_completion(operation)
            .into_completion();
        let Some(completion) = completion else {
            return Err(self.rollback(RootFailure::Unavailable));
        };
        let (_, outcome) = completion.into_parts();
        let Some(observation) = outcome.into_observed_state() else {
            return Err(self.rollback(RootFailure::Unavailable));
        };
        let (model, expected) = {
            let shared = self.shared.borrow();
            (alloc::rc::Rc::clone(&shared.model), shared.stamp)
        };
        if observation.snapshot() != ready || expected.source() != ready {
            return Err(self.rollback(RootFailure::InvalidRequest));
        }
        let binding = model.binding();
        let preparation = support::request(
            &binding,
            ResourceStateInput::Initial(&observation),
        );
        let Ok(candidate) = preparation.prepare_with(model.as_ref()) else {
            return Err(self.rollback(RootFailure::ProviderFailure));
        };
        // Native evaluation and the retained-state clone precede authority.
        let retained = candidate.state().clone();
        let current_binding = model.binding();
        let index = match self.publication_client(request) {
            Ok(index) => index,
            Err(cause) => return Err(self.rollback(cause)),
        };
        let mut shared = self.shared.borrow_mut();
        let checked = candidate.check_admission(
            current_binding,
            preparation.stamp(),
            ResourceStateTarget::Initial(shared.stamp.source()),
            preparation.limits(),
            InstallationOutcome::Current,
        );
        if checked.is_err() {
            drop(shared);
            return Err(self.rollback(RootFailure::Stale));
        }
        let proposed = candidate.into_output().state;
        let installed = shared.install_initial(&expected, proposed, retained);
        drop(shared);
        let Ok(state) = installed else {
            return Err(self.rollback(RootFailure::Stale));
        };
        // Only infallible metadata publication follows state installation.
        self.clients[index].opened = true;
        Ok(RootOpenedBuffer { state, editable: true, history: true })
    }

    fn retain_completion(
        &mut self,
        index: usize,
        ticket: RootOpenTicket,
        outcome: &Result<RootOpenedBuffer, RootOpenFailure>,
    ) -> RootOpenCompletion {
        let completion = RootOpenCompletion { ticket, outcome: *outcome };
        self.opens[index].completion = Some(completion);
        completion
    }

    fn rollback(&mut self, cause: RootFailure) -> RootOpenFailure {
        let close = self.resource.close(self.resource.close_request());
        RootOpenFailure {
            cause,
            rollback: match close {
                ResourceCloseOutcome::Closed => RollbackOutcome::Reverted,
                _ => RollbackOutcome::IncompleteShutdown,
            },
        }
    }

    pub(super) fn cancel(
        &mut self,
        ticket: RootOpenTicket,
        cause: CancellationCause,
    ) -> Result<CancellationOutcome, RootFailure> {
        self.client_index(&ticket.client)?;
        let index = self.open_index(&ticket)?;
        if self.opens[index].completion.is_some() {
            return Err(RootFailure::AlreadyDelivered);
        }
        self.opens[index].cancelled = true;
        let RootOpenTarget::Independent(resource) =
            self.opens[index].request.target
        else {
            return Ok(CancellationOutcome::new(
                cause,
                ExecutionOutcome::NeverStarted,
            ));
        };
        let cancellation =
            ResourceCancellationRequest::new(resource.operation(), cause)
                .unwrap();
        match self.resource.request_cancellation(cancellation) {
            ResourceCancellationResult::Requested(outcome)
            | ResourceCancellationResult::AlreadyRequested(outcome) => {
                Ok(outcome)
            }
            _ => Err(RootFailure::ProviderFailure),
        }
    }

    fn open_index(
        &self,
        ticket: &RootOpenTicket,
    ) -> Result<usize, RootFailure> {
        self.opens
            .iter()
            .position(|entry| {
                entry.request.client == ticket.client
                    && entry.request.request == ticket.request
            })
            .ok_or(RootFailure::UnknownRequest)
    }

    fn publication_client(
        &self,
        request: &RootOpenRequest,
    ) -> Result<usize, RootFailure> {
        let client = self.active_client(&request.client)?;
        let ticket =
            RootOpenTicket { client: request.client, request: request.request };
        let open = &self.opens[self.open_index(&ticket)?];
        if open.request != *request || open.completion.is_some() {
            return Err(RootFailure::Stale);
        }
        if open.cancelled {
            return Err(RootFailure::Cancelled);
        }
        Ok(client)
    }
}

const fn failure(cause: RootFailure) -> RootOpenFailure {
    RootOpenFailure { cause, rollback: RollbackOutcome::Reverted }
}

const fn resource_failure(cause: ResourceOpenFailure) -> RootFailure {
    match cause {
        ResourceOpenFailure::Cancelled => RootFailure::Cancelled,
        ResourceOpenFailure::Replaced | ResourceOpenFailure::Stale => {
            RootFailure::Stale
        }
        ResourceOpenFailure::Denied => RootFailure::Denied,
        ResourceOpenFailure::Unavailable => RootFailure::Unavailable,
        ResourceOpenFailure::Pressure => RootFailure::Pressure,
        _ => RootFailure::ProviderFailure,
    }
}
