use reovim_kernel_interface::interaction::delivery::update as delivery;
use reovim_kernel_interface::resource::port::ResourceAccessPort;
use reovim_kernel_interface::resource::state::ResourceStateModel;
use reovim_kernel_interface::semantic::domain::DomainPolicyPort;
use reovim_kernel_interface::semantic::representation::RepresentationPolicyPort;
use reovim_kernel_interface::semantic::snapshot::RepresentationSnapshotPort;

use crate::{Runtime, RuntimeError};

impl<A, M, D, R, S> Runtime<A, M, D, R, S>
where
    A: ResourceAccessPort,
    M: ResourceStateModel,
    D: DomainPolicyPort<State = M::State>,
    R: RepresentationPolicyPort<State = M::State>,
    S: RepresentationSnapshotPort<State = M::State>,
{
    pub(crate) fn execute_recovery(&mut self, index: usize) {
        let result = self.recovery_snapshot(index);
        let session = &mut self.sessions[index];
        match result {
            Ok(update) => {
                session.note_ready(&update);
                let work =
                    session.recovery.as_mut().expect("admitted recovery");
                work.completion = Some(update);
                session.invalidated = false;
            }
            Err(cause) => session.recovery_failed(cause),
        }
    }

    fn recovery_snapshot(
        &mut self,
        index: usize,
    ) -> Result<
        reovim_kernel_interface::interaction::update::RepresentationUpdate,
        delivery::RepresentationResynchronizationFailure,
    > {
        use delivery::RepresentationResynchronizationFailure as Failure;
        if self.sessions[index].ending() {
            return Err(Failure::Closing);
        }
        if !self.recovery_provider_current() {
            return Err(Failure::Replaced);
        }
        let receipt = self.sessions[index]
            .recovery
            .as_ref()
            .expect("admitted recovery owns its receipt")
            .request
            .receipt();
        let result = self.make_snapshot(index, Some(receipt));
        if !self.recovery_provider_current() {
            return Err(Failure::Replaced);
        }
        let update = result.map_err(|cause| match cause {
            RuntimeError::Capacity => Failure::Pressure,
            _ => Failure::RuntimeFailure,
        })?;
        receipt
            .validate_result(&update)
            .map_err(|_| Failure::RuntimeFailure)?;
        Ok(update)
    }

    fn recovery_provider_current(&self) -> bool {
        self.model.binding() == self.config.buffer.model
            && self.snapshot.descriptor() == &self.snapshot_selection
    }
}
