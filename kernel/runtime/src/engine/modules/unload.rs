use super::{Lifecycle, ResourceId, Runtime, fence_exports};
use alloc::rc::Rc;
use reovim_kernel_interface::Error;

impl Runtime {
    /// Admits or joins teardown of one exact module instance.
    /// Closing is admitted work: drive Runtime until Closed. No callback runs
    /// during admission; required cleanup keeps the instance and code alive.
    /// # Errors
    /// Busy rejects before fencing if exports or active dependants retain use.
    /// Unknown identities are unavailable; Runtime shutdown rejects new unload.
    pub fn unload_module(
        &mut self,
        identity: ResourceId,
    ) -> Result<Lifecycle, Error> {
        if self.closing {
            return Err(Error::CLOSED);
        }
        let instance = self.instance(identity)?;
        if instance.stopped() {
            instance.status.set(Lifecycle::Closed);
            return Ok(Lifecycle::Closed);
        }
        if instance.closing {
            return Ok(Lifecycle::Closing);
        }
        if instance.probe_uses != 0 {
            return Err(Error::BUSY);
        }
        if self.modules.iter().any(|consumer| {
            !consumer.stopped() && consumer.dependencies.contains(&identity)
        }) {
            return Err(Error::BUSY);
        }
        let registry = self.registry.borrow();
        if registry.entries.iter().any(|entry| {
            instance.exports.contains(&entry.identity)
                && Rc::strong_count(entry) != 1
        }) {
            return Err(Error::BUSY);
        }
        drop(registry);
        let instance = self
            .modules
            .iter_mut()
            .find(|instance| instance.resource.identity == identity)
            .ok_or(Error::UNAVAILABLE)?;
        instance.closing = true;
        instance.batch.fence();
        instance.status.set(Lifecycle::Closing);
        fence_exports(instance, &self.registry.borrow().entries);
        Ok(Lifecycle::Closing)
    }
}
