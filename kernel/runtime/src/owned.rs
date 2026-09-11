//! Client-owned provider lifetime, independent of module operation grants.

use alloc::rc::Rc;
use core::cell::RefCell;
use reovim_kernel_interface::{Error, ResourceId};

use crate::{
    Shared,
    namespace::{Kind, Scope},
};

pub fn close(
    shared: &Rc<RefCell<Shared>>,
    scope: &Rc<RefCell<Scope>>,
    key: u64,
) -> Result<(), Error> {
    let mut registry = shared.borrow_mut();
    let index = registry.entry_index(key)?;
    let ownership =
        registry.entries[index].ownership.as_ref().ok_or(Error::INVALID)?;
    if ownership.client != scope.borrow().owner {
        return Err(Error::DENIED);
    }
    let provider = fence(&mut registry.entries[index]);
    drop(registry);
    if let Some(provider) = provider {
        provider.borrow_mut().close();
    }
    reclaim(shared);
    Ok(())
}

type Provider =
    Rc<RefCell<reovim_kernel_interface::provider::ResourceProvider>>;

fn fence(entry: &mut crate::namespace::Entry) -> Option<Provider> {
    let ownership = entry.ownership.as_mut()?;
    if ownership.closed {
        return None;
    }
    ownership.closed = true;
    match &entry.kind {
        Kind::Provider(provider) => Some(provider.clone()),
        _ => None,
    }
}

pub fn close_all(shared: &Rc<RefCell<Shared>>) {
    close_matching(shared, |_| true);
}

pub fn close_owner(shared: &Rc<RefCell<Shared>>, owner: u64) {
    close_matching(shared, |client| client == owner);
    reclaim(shared);
}

fn close_matching(shared: &Rc<RefCell<Shared>>, matches: impl Fn(u64) -> bool) {
    loop {
        let provider = shared
            .borrow_mut()
            .entries
            .iter_mut()
            .filter(|entry| {
                entry
                    .ownership
                    .as_ref()
                    .is_some_and(|owner| matches(owner.client))
            })
            .find_map(fence);
        let Some(provider) = provider else { break };
        provider.borrow_mut().close();
    }
}

pub fn reclaim(shared: &Rc<RefCell<Shared>>) {
    loop {
        let mut registry = shared.borrow_mut();
        let Some(entry) = registry
            .entries
            .iter_mut()
            .find(|entry| entry.closed() && entry.kind.removable())
        else {
            break;
        };
        let retired = core::mem::replace(&mut entry.kind, Kind::Directory);
        if let Some(group) = entry.group.take() {
            group.borrow_mut().retain(|id| *id != ResourceId(entry.key));
        }
        entry.activation = None;
        entry.receiver = None;
        // Keep a bounded closed identity for idempotent close. It is hidden
        // from lookup, and cannot retarget a subsequently published name.
        drop(registry);
        drop(retired);
    }
}
