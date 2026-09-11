//! Module callbacks can publish initialized children under their own exports.

use crate::{
    Shared,
    namespace::{Entry, Kind, Scope},
};
use alloc::{rc::Rc, vec::Vec};
use core::cell::RefCell;
use reovim_kernel_interface::{
    Error, ResourceId,
    driver::Lifecycle,
    provider::{Permissions, ResourceProvider},
};

pub fn publish(
    shared: &Rc<RefCell<Shared>>,
    scope: &Rc<RefCell<Scope>>,
    parent: ResourceId,
    name: &str,
    provider: ResourceProvider,
) -> Result<ResourceId, (Error, ResourceProvider)> {
    let mut shared = shared.borrow_mut();
    let validated = validate(&shared, scope, parent, name, &provider);
    let (key, activation, group) = match validated {
        Ok(value) => value,
        Err(error) => return Err((error, provider)),
    };
    let ownership = provider.is_owned().then(|| {
        let active = shared.active.expect("validated active publication");
        let record = shared
            .requests
            .iter()
            .find(|record| record.borrow().id == active)
            .expect("validated publishing request")
            .borrow();
        crate::namespace::Ownership {
            client: record.scope.borrow().owner,
            closed: false,
        }
    });
    let mut scope_value = scope.borrow_mut();
    let mut owned = group.borrow_mut();
    if let Err(error) = reserve(&mut scope_value, &mut owned) {
        return Err((error, provider));
    }
    let global = shared.ids.take().expect("publication capacity reserved");
    let local =
        ResourceId(shared.ids.take().expect("publication name reserved"));
    let name = name.into();
    let provider = Rc::new(RefCell::new(provider));
    shared.entries.push(Entry {
        key: global,
        parent: key,
        name,
        kind: Kind::Provider(provider),
        activation: Some(activation),
        receiver: Some(scope.clone()),
        group: Some(group.clone()),
        ownership,
    });
    owned.push(ResourceId(global));
    scope_value.names.push((local, global));
    scope_value.grants.push((global, Permissions::ALL));
    Ok(local)
}

type Owner = (u64, Rc<core::cell::Cell<Lifecycle>>, crate::namespace::Group);
fn validate(
    shared: &Shared,
    scope: &Rc<RefCell<Scope>>,
    parent: ResourceId,
    name: &str,
    provider: &ResourceProvider,
) -> Result<Owner, Error> {
    if shared.closing {
        return Err(Error::CLOSED);
    }
    crate::namespace::validate(name, shared.limits.path_bytes)?;
    if name.is_empty() || name.contains('/') {
        return Err(Error::INVALID);
    }
    crate::registration::resource(provider, shared.limits)?;
    let scope_value = scope.borrow();
    let parent = scope_value.key(parent)?;
    let entry = &shared.entries[shared.entry_index(parent)?];
    let activation = entry.activation.as_ref().ok_or(Error::DENIED)?;
    let group = entry.group.as_ref().ok_or(Error::DENIED)?;
    authorize(shared, scope, activation)?;
    if entry.ownership.is_some()
        || !entry.kind.accepts_children()
        || shared.path(parent, name).is_ok()
    {
        return Err(Error::INVALID);
    }
    if shared.entries.len() == shared.limits.resources
        || scope_value.names.len() == shared.limits.resources
        || !shared.ids.can_take(2)
    {
        return Err(Error::PRESSURE);
    }
    Ok((parent, activation.clone(), group.clone()))
}
fn authorize(
    shared: &Shared,
    scope: &Rc<RefCell<Scope>>,
    activation: &Rc<core::cell::Cell<Lifecycle>>,
) -> Result<(), Error> {
    if activation.get() != Lifecycle::Ready {
        return Err(Error::NOT_READY);
    }
    let active = shared.active.ok_or(Error::DENIED)?;
    let record = shared
        .requests
        .iter()
        .find(|r| r.borrow().id == active)
        .ok_or(Error::DENIED)?
        .borrow();
    if !record.receiver.as_ref().is_some_and(|r| Rc::ptr_eq(r, scope))
        || !record
            .activation
            .as_ref()
            .is_some_and(|a| Rc::ptr_eq(a, activation))
    {
        return Err(Error::DENIED);
    }
    Ok(())
}
fn reserve(
    scope: &mut Scope,
    group: &mut Vec<ResourceId>,
) -> Result<(), Error> {
    scope.names.try_reserve(1).map_err(|_| Error::PRESSURE)?;
    scope.grants.try_reserve(1).map_err(|_| Error::PRESSURE)?;
    group.try_reserve(1).map_err(|_| Error::PRESSURE)
}

pub fn withdraw(
    shared: &Rc<RefCell<Shared>>,
    scope: &Rc<RefCell<Scope>>,
    resource: ResourceId,
) -> Result<(), Error> {
    let mut shared = shared.borrow_mut();
    let key = scope.borrow().key(resource)?;
    let entry = &shared.entries[shared.entry_index(key)?];
    let activation = entry.activation.as_ref().ok_or(Error::DENIED)?;
    let group = entry.group.clone().ok_or(Error::DENIED)?;
    authorize(&shared, scope, activation)?;
    let ids: Vec<_> = shared
        .entries
        .iter()
        .filter(|entry| shared.descends(entry.key, key))
        .map(|entry| ResourceId(entry.key))
        .collect();
    check_withdrawal(&shared, &ids, &group)?;
    let mut retired = Vec::new();
    retired.try_reserve(ids.len()).map_err(|_| Error::PRESSURE)?;
    for id in &ids {
        let index = shared.entry_index(id.0).expect("validated withdrawal");
        retired.push(shared.entries.swap_remove(index));
    }
    group.borrow_mut().retain(|id| !ids.contains(id));
    drop(shared);
    // Removing names is not a module-unload callback. Captured uses rejected
    // above; executable ownership remains with the module and retained values.
    drop(retired);
    Ok(())
}
fn check_withdrawal(
    shared: &Shared,
    ids: &[ResourceId],
    group: &crate::namespace::Group,
) -> Result<(), Error> {
    for id in ids {
        let entry = &shared.entries[shared.entry_index(id.0)?];
        if !matches!(entry.kind, Kind::Directory)
            && (!entry
                .group
                .as_ref()
                .is_some_and(|owner| Rc::ptr_eq(owner, group))
                || entry.ownership.is_some()
                || !entry.kind.removable())
        {
            return Err(Error::BUSY);
        }
    }
    Ok(())
}
