use alloc::rc::Rc;
use core::cell::RefCell;
use reovim_kernel_interface::{Error, ResourceId};

use crate::engine::Registry;

pub(super) fn exports(
    registry: &Rc<RefCell<Registry>>,
    identities: &[ResourceId],
) -> Result<(), Error> {
    for identity in identities.iter().rev() {
        drain(registry, *identity)?;
    }
    Ok(())
}

fn drain(
    registry: &Rc<RefCell<Registry>>,
    identity: ResourceId,
) -> Result<(), Error> {
    let entries = registry.borrow();
    let Some(index) =
        entries.entries.iter().position(|e| e.identity == identity)
    else {
        return Ok(());
    };
    let entry = &entries.entries[index];
    // Access, requests and factory descendants retain this exact Entry. A
    // registry reference alone proves those owners have finished with it.
    if Rc::strong_count(entry) != 1 {
        return Err(Error::BUSY);
    }
    let entry = entry.clone();
    drop(entries);
    entry.implementation.provider.borrow_mut().stop()?;
    registry.borrow_mut().entries.remove(index);
    // Neither a registry guard nor the module driver surrounds destruction.
    drop(entry);
    Ok(())
}

pub(super) fn advance(
    instance: &mut super::Instance,
    registry: &Rc<RefCell<Registry>>,
    cx: &mut core::task::Context<'_>,
) -> bool {
    if instance.stopped() {
        return false;
    }
    let started = instance.cleanup == super::Cleanup::Idle;
    instance.cleanup = super::Cleanup::Pending;
    if let Err(error) = exports(registry, &instance.exports) {
        instance.control.cleanup_failed(error);
        return started;
    }
    module(instance, cx) || started
}

fn module(
    instance: &mut super::Instance,
    cx: &mut core::task::Context<'_>,
) -> bool {
    if instance.stopped() {
        return false;
    }
    if let Err(error) = instance.batch.close() {
        instance.control.cleanup_failed(error);
        return false;
    }
    let Some(module) = instance.module.as_mut() else { return false };
    if let Err(error) = module.close() {
        instance.control.cleanup_failed(error);
    }
    let progress = super::drive(instance, false, cx);
    if instance.module.as_ref().unwrap().status()
        != reovim_kernel_interface::driver::Lifecycle::Closed
    {
        return progress;
    }
    // Destructors may still use dependencies after the driver closes.
    instance.module = None;
    if instance.parent_imports.is_some() {
        return true;
    }
    instance.cleanup = super::Cleanup::Stopped;
    if instance.closing {
        instance.status.set(reovim_kernel_interface::driver::Lifecycle::Closed);
    }
    true
}
