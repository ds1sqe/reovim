use reovim_kernel_interface::buffer::BufferIncarnationStamp;
use reovim_kernel_interface::cancellation::{
    CancellationCause, CancellationOutcome, DeadlineState, ExecutionOutcome,
};
use reovim_kernel_interface::interaction::delivery::session::{
    InteractionCloseOutcome, InteractionCloseRequest,
};
use reovim_kernel_interface::interaction::port::InteractionSessionPort;
use reovim_kernel_interface::interaction::root as contract;
use reovim_kernel_interface::lifecycle::{
    LifecycleIdentity, LifecycleState, ReadinessOutcome, ShutdownOutcome,
};
use reovim_kernel_interface::resource::access::ResourceCloseOutcome;
use reovim_kernel_interface::resource::port::ResourceAccessPort;
use reovim_kernel_interface::resource::state::ResourceStateModel;
use reovim_kernel_interface::semantic::domain::DomainPolicyPort;
use reovim_kernel_interface::semantic::representation::RepresentationPolicyPort;
use reovim_kernel_interface::semantic::snapshot::RepresentationSnapshotPort;

use crate::opening::OpenRecord;
use crate::{Runtime, Session};

impl<A, M, D, R, S> contract::InteractionRootPort for Runtime<A, M, D, R, S>
where
    A: ResourceAccessPort,
    M: ResourceStateModel,
    D: DomainPolicyPort<State = M::State>,
    R: RepresentationPolicyPort<State = M::State>,
    S: RepresentationSnapshotPort<State = M::State>,
{
    type Endpoint = Session;

    fn owner(&self) -> LifecycleIdentity {
        self.config.owner
    }
    fn state(&self) -> LifecycleState {
        self.phase
    }
    fn limits(&self) -> contract::RootLimits {
        self.config.limits
    }

    fn start(
        &mut self,
        request: contract::RootLifecycleRequest,
    ) -> Result<Option<ReadinessOutcome>, contract::RootFailure> {
        self.check_lifecycle(request)?;
        if !matches!(
            self.phase,
            LifecycleState::Constructed | LifecycleState::Ready
        ) {
            return Err(contract::RootFailure::Closing);
        }
        self.phase = LifecycleState::Ready;
        Ok(Some(ReadinessOutcome::Ready))
    }

    fn attach_client(
        &mut self,
        request: contract::RootAttachRequest,
    ) -> Result<contract::RootClientHandle, contract::RootFailure> {
        if request.root != self.config.owner {
            return Err(contract::RootFailure::Stale);
        }
        self.check_ready()?;
        let session = self
            .sessions
            .iter_mut()
            .find(|session| {
                session.binding.authorization == request.authorization
                    && session.binding.session.stamp().client()
                        == request.client
            })
            .ok_or(contract::RootFailure::Denied)?;
        if session.ending() {
            return Err(contract::RootFailure::Closed);
        }
        let handle = contract::RootClientHandle {
            root: request.root,
            client: request.client,
            attachment: session.binding.session.stamp().lifecycle(),
        };
        session.client = Some(handle);
        Ok(handle)
    }

    fn open(
        &mut self,
        request: contract::RootOpenRequest,
    ) -> Result<contract::RootOpenTicket, contract::RootFailure> {
        self.check_ready()?;
        let index = self.client_index(request.client)?;
        if let Some(previous) = self.open_index(&request) {
            return self.replay_open(previous, &request);
        }
        self.check_open(&request, index)?;
        self.opens.push(OpenRecord {
            request,
            outcome: None,
            delivered: false,
            cancellation: None,
        });
        Ok(ticket(&request))
    }

    fn take_open(
        &mut self,
        ticket: contract::RootOpenTicket,
    ) -> Result<Option<contract::RootOpenCompletion>, contract::RootFailure>
    {
        self.client_index(ticket.client)?;
        let record = self
            .opens
            .iter_mut()
            .find(|record| {
                record.request.client == ticket.client
                    && record.request.request == ticket.request
            })
            .ok_or(contract::RootFailure::UnknownRequest)?;
        if record.delivered {
            return Err(contract::RootFailure::AlreadyDelivered);
        }
        let Some(outcome) = record.outcome else {
            return Ok(None);
        };
        record.delivered = true;
        Ok(Some(contract::RootOpenCompletion { ticket, outcome }))
    }

    fn cancel_open(
        &mut self,
        ticket: contract::RootOpenTicket,
        cause: CancellationCause,
    ) -> Result<CancellationOutcome, contract::RootFailure> {
        self.client_index(ticket.client)?;
        let record = self
            .opens
            .iter_mut()
            .find(|record| {
                record.request.client == ticket.client
                    && record.request.request == ticket.request
            })
            .ok_or(contract::RootFailure::UnknownRequest)?;
        record.cancellation.get_or_insert(cause);
        let execution = if record.outcome.is_some() {
            ExecutionOutcome::Stopped
        } else {
            ExecutionOutcome::OwnedNonterminal
        };
        Ok(CancellationOutcome::new(cause, execution))
    }

    fn endpoint(
        &mut self,
        client: contract::RootClientHandle,
        buffer: BufferIncarnationStamp,
    ) -> Result<&mut Session, contract::RootFailure> {
        let index = self.client_index(client)?;
        if self.phase == LifecycleState::Closed {
            return Err(contract::RootFailure::Closed);
        }
        let session = &mut self.sessions[index];
        if session.current.is_none()
            || session.binding.session.scope().buffer() != buffer
        {
            return Err(contract::RootFailure::Denied);
        }
        Ok(session)
    }

    fn detach_client(
        &mut self,
        client: contract::RootClientHandle,
        request: contract::RootLifecycleRequest,
    ) -> Result<ShutdownOutcome, contract::RootFailure> {
        self.check_lifecycle(request)?;
        let index = self.client_index(client)?;
        self.fence_opens(Some(client));
        if self.opens.iter().any(|record| {
            record.request.client == client
                && (record.outcome.is_none() || !record.delivered)
        }) {
            self.sessions[index].phase = crate::session::Phase::Closing;
            return Ok(ShutdownOutcome::IncompleteShutdown);
        }
        let session = &mut self.sessions[index];
        let close = InteractionCloseRequest::new(
            &session.binding.session,
            request.work.value(),
        )
        .map_err(|_| contract::RootFailure::Pressure)?;
        let outcome = session.close(close);
        Ok(if outcome == InteractionCloseOutcome::Closed {
            ShutdownOutcome::Closed
        } else {
            ShutdownOutcome::IncompleteShutdown
        })
    }

    fn shutdown(
        &mut self,
        request: contract::RootLifecycleRequest,
    ) -> Result<ShutdownOutcome, contract::RootFailure> {
        self.check_lifecycle(request)?;
        if self.phase == LifecycleState::Closed {
            return Ok(ShutdownOutcome::Closed);
        }
        self.phase = LifecycleState::Closing;
        self.fence_opens(None);
        self.progress_open()
            .map_err(|_| contract::RootFailure::ProviderFailure)?;
        let mut incomplete = false;
        for index in 0..self.sessions.len() {
            incomplete |= self.detach_one(index, request)?;
        }
        if incomplete {
            return Ok(ShutdownOutcome::IncompleteShutdown);
        }
        self.buffer = None;
        if self.resource.close(self.resource_close)
            != ResourceCloseOutcome::Closed
        {
            return Ok(ShutdownOutcome::IncompleteShutdown);
        }
        self.phase = LifecycleState::Closed;
        Ok(ShutdownOutcome::Closed)
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
    fn detach_one(
        &mut self,
        index: usize,
        request: contract::RootLifecycleRequest,
    ) -> Result<bool, contract::RootFailure> {
        use contract::InteractionRootPort;
        let Some(client) = self.sessions[index].client else {
            return Ok(false);
        };
        self.detach_client(client, request)
            .map(|result| result != ShutdownOutcome::Closed)
    }

    fn fence_opens(&mut self, client: Option<contract::RootClientHandle>) {
        for record in self.opens.iter_mut().filter(|record| {
            client.is_none_or(|client| record.request.client == client)
                && record.outcome.is_none()
        }) {
            record.cancellation.get_or_insert(CancellationCause::OwnerClosing);
        }
    }

    fn replay_open(
        &mut self,
        previous: usize,
        request: &contract::RootOpenRequest,
    ) -> Result<contract::RootOpenTicket, contract::RootFailure> {
        if self.opens[previous].request != *request {
            return Err(contract::RootFailure::ConflictingRequest);
        }
        self.opens[previous].delivered = false;
        Ok(ticket(request))
    }
    fn check_lifecycle(
        &self,
        request: contract::RootLifecycleRequest,
    ) -> Result<(), contract::RootFailure> {
        if request.root != self.config.owner {
            return Err(contract::RootFailure::Stale);
        }
        if request.deadline == DeadlineState::Reached {
            return Err(contract::RootFailure::Deadline);
        }
        if request.work.value() == 0
            || request.work.value() > self.config.limits.work.value()
        {
            return Err(contract::RootFailure::Pressure);
        }
        Ok(())
    }

    fn check_ready(&self) -> Result<(), contract::RootFailure> {
        if self.phase != LifecycleState::Ready {
            return Err(contract::RootFailure::NotReady);
        }
        Ok(())
    }

    fn client_index(
        &self,
        client: contract::RootClientHandle,
    ) -> Result<usize, contract::RootFailure> {
        self.sessions
            .iter()
            .position(|session| session.client == Some(client))
            .ok_or(contract::RootFailure::Denied)
    }

    fn check_open(
        &self,
        request: &contract::RootOpenRequest,
        session: usize,
    ) -> Result<(), contract::RootFailure> {
        if self.opens.len() as u64 >= self.config.limits.opens.value()
            || request.work.value() == 0
            || request.work.value() > self.config.limits.work.value()
        {
            return Err(contract::RootFailure::Pressure);
        }
        if self.sessions[session].ending() {
            return Err(contract::RootFailure::Closed);
        }
        if self.sessions[session].current.is_some() {
            return Err(contract::RootFailure::Denied);
        }
        match request.target {
            contract::RootOpenTarget::Existing(buffer)
                if buffer != self.config.buffer.incarnation
                    || self.buffer.is_none() =>
            {
                Err(contract::RootFailure::Unavailable)
            }
            contract::RootOpenTarget::Independent(open)
                if self.buffer.is_some()
                    || !self.opens.is_empty()
                    || open.resource()
                        != self.config.buffer.source.access().resource() =>
            {
                Err(contract::RootFailure::Denied)
            }
            _ => Ok(()),
        }
    }
}

const fn ticket(
    request: &contract::RootOpenRequest,
) -> contract::RootOpenTicket {
    contract::RootOpenTicket {
        client: request.client,
        request: request.request,
    }
}
