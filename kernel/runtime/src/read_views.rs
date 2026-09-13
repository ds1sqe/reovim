//! Independent data-only readers: connection lifetime owns the unnamed root.

use crate::{
    Shared,
    namespace::{Entry, Kind, Scope},
};
use alloc::rc::Rc;
use core::cell::{Cell, RefCell};
use reovim_kernel_interface::{
    Connection, Error, ResourceId,
    provider::{Permissions, ResourceProvider},
};

pub fn acquire(
    shared: &Rc<RefCell<Shared>>,
    scope: &Rc<RefCell<Scope>>,
    subject: ResourceId,
) -> Result<Connection, Error> {
    acquire_root(shared, scope, subject).map(|(_, connection)| connection)
}

pub fn acquire_root(
    shared: &Rc<RefCell<Shared>>,
    scope: &Rc<RefCell<Scope>>,
    subject: ResourceId,
) -> Result<(u64, Connection), Error> {
    let (provider, owner) = {
        let registry = shared.borrow();
        if registry.closing && !crate::saving::continuing(&registry) {
            return Err(Error::CLOSED);
        }
        let scope = scope.borrow();
        let key = scope.key(subject)?;
        let entry = &registry.entries[registry.entry_index(key)?];
        if entry.closed() {
            return Err(Error::CLOSED);
        }
        if !scope
            .permissions(&registry, key)
            .is_some_and(|rights| rights.contains(Permissions::READ))
        {
            return Err(Error::DENIED);
        }
        crate::modules::require_ready(entry.activation.as_deref())?;
        let provider = match &entry.kind {
            Kind::Provider(provider) | Kind::ReadView { provider, .. } => {
                provider.clone()
            }
            _ => return Err(Error::UNSUPPORTED),
        };
        (provider, scope.owner)
    };
    let view = provider.borrow_mut().read_view()?;
    validate(&view)?;
    let (key, _, access) = publish(shared, owner, view)?;
    Ok((key, access))
}

fn validate(view: &ResourceProvider) -> Result<(), Error> {
    if view.is_owned()
        || view.forwarding
        || view.surfaces.is_empty()
        || view.surfaces.iter().any(|surface| {
            surface.methods != Permissions::READ
                || !surface.input.is_empty()
                || !surface.output.is_empty()
        })
    {
        return Err(Error::PROVIDER);
    }
    Ok(())
}

pub fn publish(
    shared: &Rc<RefCell<Shared>>,
    owner: u64,
    provider: ResourceProvider,
) -> Result<(u64, Rc<RefCell<Scope>>, Connection), Error> {
    let mut registry = shared.borrow_mut();
    crate::registration::resource(&provider, registry.limits)?;
    registry.scopes.retain(|scope| scope.strong_count() != 0);
    if registry.entries.len() == registry.limits.resources
        || registry.scopes.len() == registry.limits.callers
        || !registry.ids.can_take(3)
    {
        return Err(Error::PRESSURE);
    }
    registry.scopes.try_reserve(1).map_err(|_| Error::PRESSURE)?;
    let key = registry.id()?;
    let scope_id = registry.id()?;
    let local = ResourceId(registry.id()?);
    let live = Rc::new(Cell::new(true));
    let mut scope =
        Scope::new(scope_id, &[(ResourceId(key), Permissions::READ)]);
    scope.owner = owner;
    scope.root = key;
    scope.names.push((local, key));
    scope.aliases.borrow_mut().push(("requests".into(), 2));
    let scope = Rc::new(RefCell::new(scope));
    registry.scopes.push(Rc::downgrade(&scope));
    registry.entries.push(Entry {
        key,
        parent: key,
        name: "".into(),
        kind: Kind::ReadView {
            provider: Rc::new(RefCell::new(provider)),
            live: Rc::downgrade(&live),
        },
        activation: None,
        receiver: None,
        group: None,
        ownership: None,
    });
    Ok((
        key,
        scope.clone(),
        crate::access::view_connection(shared.clone(), scope, live),
    ))
}

pub fn reclaim(shared: &Rc<RefCell<Shared>>) {
    loop {
        let mut registry = shared.borrow_mut();
        let Some(index) = registry.entries.iter().position(|entry| {
            matches!(entry.kind, Kind::ReadView { .. }) && entry.closed()
        }) else {
            break;
        };
        let retired = registry.entries.remove(index);
        drop(registry);
        drop(retired);
    }
}

pub fn close(
    shared: &Shared,
    scope: u64,
    view: &Cell<bool>,
) -> Result<(), Error> {
    if shared
        .requests
        .iter()
        .any(|record| record.borrow().scope.borrow().id == scope)
    {
        return Err(Error::BUSY);
    }
    view.set(false);
    Ok(())
}
