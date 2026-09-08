use reovim_kernel_interface::interaction::delivery::update::{
    RepresentationResynchronizationAdmission,
    RepresentationResynchronizationFailure, RepresentationUpdateApplication,
};
use reovim_kernel_interface::interaction::port::InteractionSessionPort;
use reovim_kernel_interface::interaction::update::RepresentationUpdateLineage;
use reovim_kernel_interface::interaction::{
    descriptor::InteractionCapabilitySupport, notification::BufferChangePort,
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
    /// Recovers an actual lost prefix through the selected public service port.
    /// A fresh snapshot does not silently revalidate a kept local selection.
    ///
    /// # Errors
    /// Returns non-admission, terminal failure or bounded incompletion.
    pub fn recover(&mut self, index: usize) -> Result<(), DriverError> {
        let client =
            self.clients.get(index).ok_or(DriverError::UnknownClient)?;
        if client.session.capabilities().resynchronization()
            != InteractionCapabilitySupport::Supported
        {
            return Err(DriverError::Client(ClientError::Recovery(
                RepresentationResynchronizationFailure::Unsupported,
            )));
        }
        if !client.recovery_pending() {
            self.admit_recovery(index)?;
        }
        super::bounded(|| self.poll_recovery(index))?;
        self.reset_change_frontier(index)
    }

    fn admit_recovery(&mut self, index: usize) -> Result<(), DriverError> {
        self.find_gap(index)?;
        let request = self.clients[index]
            .recovery_request()
            .map_err(DriverError::Client)?;
        let admission =
            self.endpoint(index)?.request_resynchronization(request);
        let receipt = match admission {
            RepresentationResynchronizationAdmission::Admitted(receipt) => {
                receipt
            }
            RepresentationResynchronizationAdmission::NotAdmitted {
                cause,
                ..
            } => {
                return Err(DriverError::Client(ClientError::Recovery(cause)));
            }
        };
        self.clients[index]
            .recovery_admitted(receipt)
            .map_err(DriverError::Client)
    }

    fn find_gap(&mut self, index: usize) -> Result<(), DriverError> {
        super::bounded(|| self.poll_gap(index))
    }

    fn poll_gap(&mut self, index: usize) -> Result<Option<()>, DriverError> {
        if self
            .clients
            .get(index)
            .ok_or(DriverError::UnknownClient)?
            .recovery_request()
            .is_ok()
        {
            return Ok(Some(()));
        }
        self.acquire_gap(index)?;
        Ok(None)
    }

    fn acquire_gap(&mut self, index: usize) -> Result<(), DriverError> {
        self.progress()?;
        let update = self.next_update(index)?;
        let identity = update.stamp().identity();
        let gap =
            matches!(update.lineage(), RepresentationUpdateLineage::Gap { .. });
        self.clients[index]
            .abandon_update(&update)
            .map_err(DriverError::Client)?;
        if gap {
            self.clients[index].record_gap(update);
        }
        self.acknowledge(
            index,
            &identity,
            RepresentationUpdateApplication::MissingPredecessor,
        )
    }

    fn poll_recovery(
        &mut self,
        index: usize,
    ) -> Result<Option<()>, DriverError> {
        self.progress()?;
        let update = match self.next_update(index) {
            Ok(update) => update,
            Err(DriverError::Delivery) => return Ok(None),
            Err(cause) => return Err(cause),
        };
        let identity = update.stamp().identity();
        let result = self.clients[index].install_recovery(update);
        let application = if result.is_ok() {
            RepresentationUpdateApplication::Installed
        } else {
            RepresentationUpdateApplication::MissingPredecessor
        };
        self.acknowledge(index, &identity, application)?;
        result.map(|_| Some(())).map_err(DriverError::Client)
    }

    fn reset_change_frontier(
        &mut self,
        index: usize,
    ) -> Result<(), DriverError> {
        let Some(subscription) = self.clients[index].subscription().cloned()
        else {
            return Ok(());
        };
        let current = self
            .endpoint(index)?
            .resynchronize_changes(&subscription)
            .map_err(|_| DriverError::Client(ClientError::Stale))?;
        self.clients[index]
            .bind_subscription(current)
            .map_err(DriverError::Client)
    }
}
