use super::super::super::{Entry, Registry};
use super::super::intern::{self, Key};
use alloc::{
    boxed::Box,
    rc::{Rc, Weak},
};
use core::cell::{Cell, RefCell};
use reovim_kernel_interface::{Error, ResourceId, provider::ResourceProvider};

pub(super) struct Stage {
    registry: Weak<RefCell<Registry>>,
    implementation: Rc<crate::engine::Implementation>,
    parent: Rc<Entry>,
    identity: ResourceId,
    maximum: usize,
    primary: Option<Rc<Entry>>,
    key: Option<Rc<Key>>,
    closed: bool,
    reserved: bool,
}
impl Stage {
    pub fn subject(&self) -> Result<Rc<Entry>, Error> {
        self.primary.clone().ok_or(Error::PROVIDER)
    }
    pub const fn new(
        registry: Weak<RefCell<Registry>>,
        parent: Rc<Entry>,
        implementation: Rc<crate::engine::Implementation>,
        identity: ResourceId,
        maximum: usize,
    ) -> Self {
        Self {
            registry,
            implementation,
            parent,
            identity,
            maximum,
            primary: None,
            key: None,
            closed: false,
            reserved: true,
        }
    }
    pub fn commit(&mut self) -> Result<Rc<Entry>, Error> {
        let entry = self.primary.as_ref().ok_or(Error::PROVIDER)?.clone();
        let key = self.key.as_ref().ok_or(Error::PROVIDER)?;
        let registry = self.registry.upgrade().ok_or(Error::PROVIDER)?;
        let mut registry = registry.borrow_mut();
        registry.install(entry.clone());
        key.reserved.set(false);
        key.creating.set(false);
        *key.ready.borrow_mut() = Rc::downgrade(&entry);
        unstage(&mut registry, self.identity);
        self.primary = None;
        self.closed = true;
        self.reserved = false;
        Ok(entry)
    }
}
pub(super) fn stage(
    owner: &Weak<RefCell<Stage>>,
    bytes: &[u8],
    provider: ResourceProvider,
) -> Result<(), (Error, ResourceProvider)> {
    let Some(owner) = owner.upgrade() else {
        return Err((Error::CLOSED, provider));
    };
    let mut state = owner.borrow_mut();
    if state.closed {
        return Err((Error::CLOSED, provider));
    }
    if state.primary.is_some() {
        return Err((Error::CONFLICT, provider));
    }
    if bytes.len() > state.maximum {
        return Err((Error::PRESSURE, provider));
    }
    let Some(registry) = state.registry.upgrade() else {
        return Err((Error::CLOSED, provider));
    };
    let mut registry = registry.borrow_mut();
    if let Err(error) = registry.validate(&[], provider.interfaces()) {
        return Err((error, provider));
    }
    let root = state
        .parent
        .namespace_owner
        .as_ref()
        .and_then(Weak::upgrade)
        .unwrap_or_else(|| state.parent.clone());
    let key = match intern::claim(&mut registry, &root, state.identity, bytes) {
        Ok(key) => key,
        Err(error) => return Err((error, provider)),
    };
    let entry = Rc::new(Entry {
        closed: Cell::new(false),
        identity: state.identity,
        path: Box::new([]),
        implementation: crate::engine::Implementation::with_origin(
            provider,
            Some(state.implementation.clone()),
        ),
        imports: state.parent.imports.clone(),
        _origin: Some(state.parent.clone()),
        namespace_owner: Some(Rc::downgrade(&root)),
        directory: false,
        parent: None,
        publication: state.parent.publication,
    });
    registry.staged.push(Rc::downgrade(&entry));
    state.primary = Some(entry);
    state.key = Some(key);
    Ok(())
}
pub(super) fn finish(state: &Rc<RefCell<Stage>>) -> Result<(), Error> {
    state.borrow_mut().closed = true;
    let primary = state.borrow().primary.clone();
    if let Some(primary) = primary {
        primary.implementation.provider.borrow_mut().stop()?;
        state.borrow_mut().primary = None;
        drop(primary);
    }
    let mut state = state.borrow_mut();
    if !state.reserved {
        return Ok(());
    }
    let registry = state.registry.upgrade().ok_or(Error::CLOSED)?;
    let mut registry = registry.borrow_mut();
    if let Some(key) = &state.key {
        key.creating.set(false);
        key.reserved.set(false);
        registry.keys.retain(|other| !Rc::ptr_eq(other, key));
    }
    unstage(&mut registry, state.identity);
    registry.release();
    state.reserved = false;
    Ok(())
}
fn unstage(registry: &mut Registry, identity: ResourceId) {
    registry.staged.retain(|entry| {
        entry.upgrade().is_some_and(|entry| entry.identity != identity)
    });
}
