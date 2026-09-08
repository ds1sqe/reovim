use reovim_kernel_interface::bound::CheckedCount;
use reovim_kernel_interface::interaction::delivery::update::{
    RepresentationUpdateAcknowledgement,
    RepresentationUpdateAcknowledgementResult, RepresentationUpdateApplication,
    RepresentationUpdateRequest,
};
use reovim_kernel_interface::interaction::identity::RepresentationUpdateCursor;
use reovim_kernel_interface::interaction::notification::{
    BufferChangeDeliveryStatus, BufferChangePort,
};
use reovim_kernel_interface::interaction::port::InteractionSessionPort;
use reovim_kernel_interface::interaction::update::{
    RepresentationUpdate, RepresentationUpdateIdentityStamp,
};
use reovim_kernel_interface::resource::state::ResourceStateModel;
use reovim_kernel_interface::semantic::domain::DomainPolicyPort;
use reovim_kernel_interface::semantic::representation::RepresentationPolicyPort;
use reovim_kernel_interface::semantic::snapshot::RepresentationSnapshotPort;

use crate::assembly::Service;
use crate::client::ClientError;
use crate::driver::DriverError;

impl<M, D, R, S> Service<M, D, R, S>
where
    M: ResourceStateModel,
    D: DomainPolicyPort<State = M::State>,
    R: RepresentationPolicyPort<State = M::State>,
    S: RepresentationSnapshotPort<State = M::State>,
{
    /// Opts this consumer into actual bounded change/snapshot pairing.
    /// Existing independent subscription consumers are not silently installed.
    ///
    /// # Errors
    /// Rejects mismatched checkpoint, unsupported observation or pressure.
    pub fn observe_changes(
        &mut self,
        index: usize,
        capacity: CheckedCount,
    ) -> Result<(), DriverError> {
        let subscription = self
            .endpoint(index)?
            .subscribe_changes(capacity)
            .map_err(|_| DriverError::Client(ClientError::Binding))?;
        self.clients[index]
            .bind_subscription(subscription)
            .map_err(DriverError::Client)
    }

    /// Consumes one notification, independent of snapshot delivery order.
    ///
    /// # Errors
    /// Returns lost continuity, stale scope, or a failed local rendezvous.
    pub fn receive_change(
        &mut self,
        index: usize,
    ) -> Result<Option<RepresentationUpdateIdentityStamp>, DriverError> {
        let client =
            self.clients.get(index).ok_or(DriverError::UnknownClient)?;
        if client.change_pending() {
            return Ok(None);
        }
        let subscription =
            client.subscription().ok_or(DriverError::Delivery)?.clone();
        let after =
            client.change_frontier().ok_or(DriverError::Delivery)?.version();
        let delivery = self
            .endpoint(index)?
            .take_change(&subscription, after)
            .map_err(|_| DriverError::Client(ClientError::Stale))?;
        match delivery.status() {
            BufferChangeDeliveryStatus::Changed => {}
            BufferChangeDeliveryStatus::Pending => return Ok(None),
            BufferChangeDeliveryStatus::Gap => {
                self.clients[index].lose_correspondence();
                return Err(DriverError::Client(ClientError::Invalidated));
            }
            BufferChangeDeliveryStatus::Closed => {
                return Err(DriverError::Delivery);
            }
        }
        let notice = delivery.notification().ok_or(DriverError::Delivery)?;
        let result = self.clients[index]
            .accept_change(notice)
            .map_err(DriverError::Client)?;
        if let Some(identity) = &result {
            self.acknowledge(
                index,
                identity,
                RepresentationUpdateApplication::Installed,
            )?;
        }
        Ok(result)
    }

    /// Consumes one actual update, retaining it if acceptance arrives later.
    ///
    /// # Errors
    /// Returns invalid lineage, lost continuity or bounded pairing pressure.
    pub fn receive_update(
        &mut self,
        index: usize,
    ) -> Result<Option<RepresentationUpdateIdentityStamp>, DriverError> {
        let client =
            self.clients.get(index).ok_or(DriverError::UnknownClient)?;
        if client.update_pending() {
            return Err(DriverError::Client(ClientError::Pressure));
        }
        let paired = client.subscription().is_some();
        let update = self.next_update(index)?;
        let identity = update.stamp().identity();
        let result = if paired {
            self.clients[index].accept_update(update)
        } else {
            self.clients[index].install(update).map(Some)
        };
        match result {
            Ok(Some(installed)) => {
                self.acknowledge(
                    index,
                    &installed,
                    RepresentationUpdateApplication::Installed,
                )?;
                Ok(Some(installed))
            }
            Ok(None) => Ok(None),
            Err(cause) => {
                self.acknowledge(
                    index,
                    &identity,
                    RepresentationUpdateApplication::MissingPredecessor,
                )?;
                Err(DriverError::Client(cause))
            }
        }
    }

    pub(super) fn next_update(
        &mut self,
        index: usize,
    ) -> Result<RepresentationUpdate, DriverError> {
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
        let client = &self.clients[index];
        let _ = self.telemetry.received_update(
            &client.session,
            &client.attachment,
            &update,
        );
        Ok(update)
    }

    pub(super) fn acknowledge(
        &mut self,
        index: usize,
        identity: &RepresentationUpdateIdentityStamp,
        application: RepresentationUpdateApplication,
    ) -> Result<(), DriverError> {
        let client =
            self.clients.get(index).ok_or(DriverError::UnknownClient)?;
        if let (RepresentationUpdateApplication::Installed, Some(state)) =
            (application, client.state())
        {
            let _ = self.telemetry.installed_update(
                client.session.stamp().handle(),
                *identity,
                &state,
            );
        }
        let ack = RepresentationUpdateAcknowledgement::new(
            &client.session,
            &client.attachment,
            *identity,
            application,
        )
        .map_err(|_| DriverError::Delivery)?;
        if self.endpoint(index)?.acknowledge_update(ack)
            != RepresentationUpdateAcknowledgementResult::Accepted
        {
            return Err(DriverError::Delivery);
        }
        Ok(())
    }
}
