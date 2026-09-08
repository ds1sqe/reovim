use reovim_kernel_interface::buffer::BufferIncarnationStamp;
use reovim_kernel_interface::cancellation::{
    CancellationCause, CancellationOutcome, DeadlineState,
};
use reovim_kernel_interface::interaction::delivery::session::{
    InteractionCloseOutcome, InteractionCloseRequest,
};
use reovim_kernel_interface::interaction::port::InteractionSessionPort;
use reovim_kernel_interface::interaction::root::{
    InteractionRootPort, RootAttachRequest, RootClientHandle, RootFailure,
    RootLifecycleRequest, RootLimits, RootOpenCompletion, RootOpenRequest,
    RootOpenTicket,
};
use reovim_kernel_interface::lifecycle::{
    Generation, LifecycleIdentity, LifecycleState, ReadinessOutcome,
    RollbackOutcome, ShutdownOutcome,
};
use reovim_kernel_interface::resource::access::ResourceCloseOutcome;
use reovim_kernel_interface::resource::port::ResourceAccessPort;

use super::{Client, Root, Session};
use crate::first_edit::Family;

impl<Model: Family> InteractionRootPort for Root<Model>
where
    Model::State: Clone,
{
    type Endpoint = Session<Model>;

    fn owner(&self) -> LifecycleIdentity {
        self.owner
    }
    fn state(&self) -> LifecycleState {
        self.state
    }
    fn limits(&self) -> RootLimits {
        self.limits
    }

    fn start(
        &mut self,
        request: RootLifecycleRequest,
    ) -> Result<Option<ReadinessOutcome>, RootFailure> {
        self.check_budget(request)?;
        if request.deadline == DeadlineState::Reached
            && self.state == LifecycleState::Starting
        {
            self.state = LifecycleState::Closed;
            return Ok(Some(ReadinessOutcome::ActivationFailed(
                RollbackOutcome::Reverted,
            )));
        }
        if request.deadline == DeadlineState::Reached {
            return Err(RootFailure::Deadline);
        }
        match self.state {
            LifecycleState::Constructed => {
                self.state = LifecycleState::Starting;
                Ok(None)
            }
            LifecycleState::Starting => {
                self.state = LifecycleState::Ready;
                Ok(Some(ReadinessOutcome::Ready))
            }
            LifecycleState::Ready => Ok(Some(ReadinessOutcome::Ready)),
            LifecycleState::Closed => Err(RootFailure::Closed),
            _ => Err(RootFailure::Closing),
        }
    }

    fn attach_client(
        &mut self,
        request: RootAttachRequest,
    ) -> Result<RootClientHandle, RootFailure> {
        self.require_ready()?;
        if request.root != self.owner {
            return Err(RootFailure::Stale);
        }
        if !self.authorizations.iter().any(|grant| {
            request.authorization == grant.authorization
                && grant.client == request.client
        }) {
            return Err(RootFailure::Denied);
        }
        if let Some(client) = self
            .clients
            .iter()
            .find(|entry| entry.handle.client == request.client)
        {
            return (!client.closing)
                .then_some(client.handle)
                .ok_or(RootFailure::Closing);
        }
        if self.clients.len() as u64 >= self.limits.clients.value() {
            return Err(RootFailure::Pressure);
        }
        let handle = RootClientHandle {
            root: self.owner,
            client: request.client,
            attachment: LifecycleIdentity::new(
                8300,
                request.client.client().value(),
                Generation::new(1),
            ),
        };
        let endpoint = Session::new(self.shared.clone(), request.client);
        self.clients.push(Client {
            handle,
            endpoint,
            opened: false,
            closing: false,
        });
        Ok(handle)
    }

    fn open(
        &mut self,
        request: RootOpenRequest,
    ) -> Result<RootOpenTicket, RootFailure> {
        self.admit_open(&request)
    }

    fn take_open(
        &mut self,
        ticket: RootOpenTicket,
    ) -> Result<Option<RootOpenCompletion>, RootFailure> {
        self.finish_open(ticket)
    }

    fn cancel_open(
        &mut self,
        ticket: RootOpenTicket,
        cause: CancellationCause,
    ) -> Result<CancellationOutcome, RootFailure> {
        self.cancel(ticket, cause)
    }

    fn endpoint(
        &mut self,
        client: RootClientHandle,
        buffer: BufferIncarnationStamp,
    ) -> Result<&mut Self::Endpoint, RootFailure> {
        let index = self.client_index(&client)?;
        if self.state == LifecycleState::Closed {
            return Err(RootFailure::Closed);
        }
        if !self.clients[index].opened {
            return Err(RootFailure::NotReady);
        }
        if self.shared.borrow().stamp.buffer() != buffer {
            return Err(RootFailure::Denied);
        }
        Ok(&mut self.clients[index].endpoint)
    }

    fn detach_client(
        &mut self,
        client: RootClientHandle,
        budget: RootLifecycleRequest,
    ) -> Result<ShutdownOutcome, RootFailure> {
        self.check_budget(budget)?;
        let index = self.client_index(&client)?;
        self.clients[index].closing = true;
        let tickets = self
            .opens
            .iter()
            .filter(|entry| {
                entry.request.client == client && entry.completion.is_none()
            })
            .map(|entry| RootOpenTicket {
                client,
                request: entry.request.request,
            })
            .collect::<alloc::vec::Vec<_>>();
        for ticket in &tickets {
            let _ = self.cancel(*ticket, CancellationCause::OwnerClosing);
        }
        let endpoint = &mut self.clients[index].endpoint;
        let close = InteractionCloseRequest::new(
            endpoint.descriptor(),
            budget.work.value(),
        )
        .map_err(|_| RootFailure::Pressure)?;
        let outcome = endpoint.close(close);
        if !tickets.is_empty()
            || !matches!(outcome, InteractionCloseOutcome::Closed)
        {
            return Ok(ShutdownOutcome::IncompleteShutdown);
        }
        Ok(ShutdownOutcome::Closed)
    }

    fn shutdown(
        &mut self,
        budget: RootLifecycleRequest,
    ) -> Result<ShutdownOutcome, RootFailure> {
        self.check_budget(budget)?;
        if self.state == LifecycleState::Closed {
            return Ok(ShutdownOutcome::Closed);
        }
        self.state = LifecycleState::Closing;
        let handles = self
            .clients
            .iter()
            .map(|entry| entry.handle)
            .collect::<alloc::vec::Vec<_>>();
        let mut incomplete = false;
        for handle in handles {
            incomplete |= self.detach_client(handle, budget)?
                == ShutdownOutcome::IncompleteShutdown;
        }
        if incomplete {
            self.state = LifecycleState::IncompleteShutdown;
            return Ok(ShutdownOutcome::IncompleteShutdown);
        }
        let request = self.resource.close_request();
        incomplete |= !matches!(
            self.resource.close(request),
            ResourceCloseOutcome::Closed
        );
        if incomplete {
            self.state = LifecycleState::IncompleteShutdown;
            return Ok(ShutdownOutcome::IncompleteShutdown);
        }
        self.state = LifecycleState::Closed;
        Ok(ShutdownOutcome::Closed)
    }
}
