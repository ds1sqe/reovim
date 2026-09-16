use alloc::rc::{Rc, Weak};
use core::cell::RefCell;
use reovim_kernel_interface::{
    Error, ResourceId, declaration::Declaration, provider::ResourceProvider,
    resource::AccessId,
};

use super::{Entry, Handle, Opened, Registry, Scope, Stage};
use crate::engine::{access::Ownership, registry};

pub(super) fn reserve(
    caller: &mut Scope,
    current: &Rc<RefCell<Scope>>,
    entry: &Entry,
    registry: &Rc<RefCell<Registry>>,
    count: usize,
) -> Result<(ResourceId, u64), Error> {
    if count == 0 {
        return Ok((registry.borrow_mut().reserve()?, 0));
    }
    let imports = entry
        .imports
        .as_ref()
        .and_then(Weak::upgrade)
        .ok_or(Error::UNSUPPORTED)?;
    if Rc::ptr_eq(&imports, current) {
        return reserve_in(caller, registry, count);
    }
    reserve_in(
        &mut *imports.try_borrow_mut().map_err(|_| Error::BUSY)?,
        registry,
        count,
    )
}

fn reserve_in(
    scope: &mut Scope,
    registry: &Rc<RefCell<Registry>>,
    count: usize,
) -> Result<(ResourceId, u64), Error> {
    if scope.closed {
        return Err(Error::CLOSED);
    }
    if count
        > scope.limits.handles - scope.handles.len() - scope.reserved_handles
    {
        return Err(Error::PRESSURE);
    }
    let first = scope.next_handle;
    let end = first
        .checked_add(u64::try_from(count).map_err(|_| Error::PRESSURE)?)
        .ok_or(Error::PRESSURE)?;
    let identity = registry.borrow_mut().reserve_batch(count + 1)?;
    scope.next_handle = end;
    scope.reserved_handles += count;
    Ok((identity, first))
}

pub(super) fn release_credits(
    caller: &mut Scope,
    current: &Rc<RefCell<Scope>>,
    imports: Option<&Rc<RefCell<Scope>>>,
    count: usize,
) {
    let Some(imports) = imports else { return };
    if Rc::ptr_eq(imports, current) {
        caller.reserved_handles -= count;
    } else {
        imports.borrow_mut().reserved_handles -= count;
    }
}

pub(super) fn stage(
    owner: &Weak<RefCell<Stage>>,
    provider: ResourceProvider,
) -> Result<AccessId, (Error, ResourceProvider)> {
    let Some(owner) = owner.upgrade() else {
        return Err((Error::CLOSED, provider));
    };
    let mut staging = owner.borrow_mut();
    let index = match vacancy(&staging, provider.interfaces()) {
        Ok(index) => index,
        Err(error) => return Err((error, provider)),
    };
    let slot = &staging.auxiliary[index];
    let token = slot.token;
    let entry = super::entry(&staging, slot.identity, provider);
    let handle = Handle {
        token,
        implementation: entry.implementation.clone(),
        entry,
        opened: Rc::new(RefCell::new(Opened {
            held: staging.ceiling,
            ceiling: staging.ceiling,
            create_access: 0,
            closed: false,
            ownership: Ownership::Owned { retain_loans: staging.retain_loans },
        })),
        interface: None,
        provisional: None,
    };
    staging.auxiliary[index].handle = Some(handle);
    Ok(token)
}

fn vacancy(
    staging: &Stage,
    interfaces: &[Declaration],
) -> Result<usize, Error> {
    if staging.closed {
        return Err(Error::CLOSED);
    }
    let index = staging
        .auxiliary
        .iter()
        .position(|slot| slot.handle.is_none())
        .ok_or(Error::PRESSURE)?;
    compatible(staging, interfaces)?;
    Ok(index)
}

pub(super) fn compatible(
    staging: &Stage,
    interfaces: &[Declaration],
) -> Result<(), Error> {
    let registry = staging.registry.upgrade().ok_or(Error::CLOSED)?;
    registry.borrow().validate(&[], interfaces)?;
    if let Some(primary) = &staging.primary {
        registry::compatible(primary, &[], interfaces)?;
    }
    for auxiliary in &staging.auxiliary {
        if let Some(handle) = &auxiliary.handle {
            registry::compatible(&handle.entry, &[], interfaces)?;
        }
    }
    Ok(())
}

pub(super) fn validate_returned(
    staging: &Stage,
    registry: &Registry,
) -> Result<(), Error> {
    for auxiliary in &staging.auxiliary {
        if auxiliary.returned {
            let handle = auxiliary.handle.as_ref().ok_or(Error::PROVIDER)?;
            registry.validate(&[], &handle.entry.implementation.interfaces)?;
        }
    }
    Ok(())
}

pub(super) fn install(staging: &mut Stage, registry: &mut Registry) -> usize {
    let mut installed = 0;
    for auxiliary in &mut staging.auxiliary {
        if auxiliary.returned {
            registry.install(auxiliary.handle.take().unwrap().entry);
            installed += 1;
        }
    }
    installed
}

pub(super) fn finish(owner: &Rc<RefCell<Stage>>) -> Result<(), Error> {
    let count = owner.borrow().auxiliary.len();
    let mut busy = None;
    for index in 0..count {
        if let Err(error) = finish_one(owner, index) {
            busy = Some(error);
        }
    }
    busy.map_or(Ok(()), Err)
}

fn finish_one(owner: &Rc<RefCell<Stage>>, index: usize) -> Result<(), Error> {
    let entry = owner.borrow().auxiliary[index]
        .handle
        .as_ref()
        .map(|handle| handle.entry.clone());
    let Some(entry) = entry else { return Ok(()) };
    entry.implementation.provider.borrow_mut().stop()?;
    owner.borrow_mut().auxiliary[index].handle.take();
    drop(entry);
    Ok(())
}
