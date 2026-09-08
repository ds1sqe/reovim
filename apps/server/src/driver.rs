//! Bounded public-port consumer; progress never supplies accepted state.

use core::fmt;

use reovim_kernel_interface::cancellation::DeadlineState;
use reovim_kernel_interface::interaction::delivery::intent::{
    InteractionOutcomeDeliveryStatus, InteractionOutcomeRequest,
    InteractionOutcomeUnavailable,
};
use reovim_kernel_interface::interaction::delivery::update::{
    RepresentationUpdateAcknowledgement,
    RepresentationUpdateAcknowledgementResult, RepresentationUpdateApplication,
    RepresentationUpdateRequest,
};
use reovim_kernel_interface::interaction::identity::{
    InteractionRequestIdentity, RepresentationUpdateCursor,
};
use reovim_kernel_interface::interaction::outcome::InteractionOutcome;
use reovim_kernel_interface::interaction::port::InteractionSessionPort;
use reovim_kernel_interface::interaction::root::{
    InteractionRootPort, RootFailure, RootLifecycleRequest, RootOpenFailure,
    RootOpenRequest, RootOpenTarget,
};
use reovim_kernel_interface::lifecycle::{ReadinessOutcome, ShutdownOutcome};
use reovim_kernel_interface::resource::state::ResourceStateModel;
use reovim_kernel_interface::semantic::domain::DomainPolicyPort;
use reovim_kernel_interface::semantic::representation::RepresentationPolicyPort;
use reovim_kernel_interface::semantic::snapshot::RepresentationSnapshotPort;
use reovim_kernel_runtime::{RuntimeError, Session};
use reovim_resource_local::profile;

use crate::assembly::Service;
use crate::client::ClientError;

/// Maximum synchronous progress rounds for this local consumer invocation.
pub const PROGRESS_ROUNDS: u64 = 64;

/// Failure is not cancellation: admitted work remains owned by the Runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DriverError {
    /// No selected consumer has this index.
    UnknownClient,
    /// Public client binding or correspondence rejected the operation.
    Client(ClientError),
    /// Root rejected lifecycle, authority or lookup.
    Root(RootFailure),
    /// Failed open retains its explicit rollback disposition.
    Open(RootOpenFailure),
    /// Bounded execution encountered a provider/configuration failure.
    Progress(RuntimeError),
    /// The selected owner's work did not become terminal within this budget.
    Incomplete,
    /// An expected public result was missing or structurally inconsistent.
    Delivery,
}

impl fmt::Display for DriverError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "reference driver: {self:?}")
    }
}
impl core::error::Error for DriverError {}

impl<M, D, R, S> Service<M, D, R, S>
where
    M: ResourceStateModel,
    D: DomainPolicyPort<State = M::State>,
    R: RepresentationPolicyPort<State = M::State>,
    S: RepresentationSnapshotPort<State = M::State>,
{
    /// Boots and opens this consumer; later consumers explicitly join Buffer.
    ///
    /// # Errors
    /// Returns failed readiness/open, bounded incompletion or invalid binding.
    pub fn open(&mut self, index: usize) -> Result<(), DriverError> {
        let ready =
            self.runtime.start(self.lifecycle()).map_err(DriverError::Root)?;
        if ready != Some(ReadinessOutcome::Ready) {
            return Err(DriverError::Incomplete);
        }
        let target = self
            .clients
            .iter()
            .find_map(super::client::Client::state)
            .map_or(self.open_target, |state| {
                RootOpenTarget::Existing(state.buffer())
            });
        let client =
            self.clients.get(index).ok_or(DriverError::UnknownClient)?;
        let handle = self
            .runtime
            .attach_client(client.attach_request(self.runtime.owner()))
            .map_err(DriverError::Root)?;
        let ticket = self
            .runtime
            .open(RootOpenRequest {
                client: handle,
                request: InteractionRequestIdentity::new(1),
                target,
                work: self.runtime.limits().work,
            })
            .map_err(DriverError::Root)?;
        let completion = bounded(|| {
            self.progress()?;
            self.runtime.take_open(ticket).map_err(DriverError::Root)
        })?;
        let opened = completion.outcome.map_err(DriverError::Open)?;
        let client = &mut self.clients[index];
        client.opened(handle, opened).map_err(DriverError::Client)?;
        let port = self
            .runtime
            .endpoint(handle, opened.state.buffer())
            .map_err(DriverError::Root)?;
        client.initialize(port).map_err(DriverError::Client)
    }

    /// Submits an observed semantic edit and retrieves its actual outcome.
    /// Updates remain separately available to this or other consumers.
    ///
    /// # Errors
    /// Returns invalid binding, failed progress or bounded incompletion.
    pub fn edit(
        &mut self,
        index: usize,
        request: u128,
        payload: &[u8],
    ) -> Result<InteractionOutcome, DriverError> {
        let client =
            self.clients.get(index).ok_or(DriverError::UnknownClient)?;
        let intent =
            client.intent(request, payload).map_err(DriverError::Client)?;
        let lookup =
            InteractionOutcomeRequest::new(&client.session, intent.stamp())
                .map_err(|_| DriverError::Delivery)?;
        let admission = self.endpoint(index)?.submit_intent(intent);
        if let Some(refusal) = admission.non_admission() {
            return Ok(refusal.outcome());
        }
        bounded(|| self.poll_outcome(index, &lookup))
    }

    fn poll_outcome(
        &mut self,
        index: usize,
        lookup: &InteractionOutcomeRequest,
    ) -> Result<Option<InteractionOutcome>, DriverError> {
        self.progress()?;
        let delivery = self.endpoint(index)?.take_outcome(*lookup);
        if let Some(outcome) = delivery.outcome() {
            return Ok(Some(outcome));
        }
        let pending = InteractionOutcomeDeliveryStatus::Unavailable(
            InteractionOutcomeUnavailable::Pending,
        );
        if delivery.status() != pending {
            return Err(DriverError::Delivery);
        }
        Ok(None)
    }

    /// Consumes one queued semantic snapshot and acknowledges installation.
    ///
    /// # Errors
    /// Returns missing delivery, stale state or failed acknowledgement.
    pub fn receive(&mut self, index: usize) -> Result<(), DriverError> {
        let client =
            self.clients.get(index).ok_or(DriverError::UnknownClient)?;
        let next =
            client.cursor().checked_add(1).ok_or(DriverError::Delivery)?;
        let request = RepresentationUpdateRequest::new(
            &client.session,
            &client.attachment,
            RepresentationUpdateCursor::new(next),
        )
        .map_err(|_| DriverError::Delivery)?;
        let update = self
            .endpoint(index)?
            .take_update(request)
            .into_update()
            .ok_or(DriverError::Delivery)?;
        let client = &mut self.clients[index];
        let stamp = client.install(update).map_err(DriverError::Client)?;
        let ack = RepresentationUpdateAcknowledgement::new(
            &client.session,
            &client.attachment,
            stamp,
            RepresentationUpdateApplication::Installed,
        )
        .map_err(|_| DriverError::Delivery)?;
        if self.endpoint(index)?.acknowledge_update(ack)
            != RepresentationUpdateAcknowledgementResult::Accepted
        {
            return Err(DriverError::Delivery);
        }
        Ok(())
    }

    /// Borrows the same public Session used by the executing consumer.
    ///
    /// # Errors
    /// Returns unopened, unknown, replaced or denied attachment failure.
    pub fn endpoint(
        &mut self,
        index: usize,
    ) -> Result<&mut Session, DriverError> {
        let client =
            self.clients.get(index).ok_or(DriverError::UnknownClient)?;
        let handle = client.handle().ok_or(DriverError::Delivery)?;
        self.runtime
            .endpoint(handle, client.session.scope().buffer())
            .map_err(DriverError::Root)
    }

    /// Drives only the Runtime's declared work quantum, not a scripted result.
    ///
    /// # Errors
    /// Returns the exact bounded Runtime preparation/configuration failure.
    pub fn progress(&mut self) -> Result<(), DriverError> {
        self.runtime
            .progress(profile::count(1))
            .map_err(DriverError::Progress)?;
        Ok(())
    }

    /// Requests ordered shutdown without discarding undelivered terminals.
    ///
    /// # Errors
    /// Returns an invalid lifecycle or explicit incomplete ownership.
    pub fn close(&mut self) -> Result<(), DriverError> {
        if self.runtime.shutdown(self.lifecycle()).map_err(DriverError::Root)?
            != ShutdownOutcome::Closed
        {
            return Err(DriverError::Incomplete);
        }
        Ok(())
    }

    fn lifecycle(&self) -> RootLifecycleRequest {
        RootLifecycleRequest {
            root: self.runtime.owner(),
            work: self.runtime.limits().work,
            deadline: DeadlineState::Pending,
        }
    }
}

fn bounded<T>(
    mut step: impl FnMut() -> Result<Option<T>, DriverError>,
) -> Result<T, DriverError> {
    for _ in 0..PROGRESS_ROUNDS {
        if let Some(value) = step()? {
            return Ok(value);
        }
    }
    Err(DriverError::Incomplete)
}
