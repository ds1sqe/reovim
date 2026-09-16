use alloc::{
    boxed::Box,
    rc::{Rc, Weak},
    vec::Vec,
};
use core::cell::RefCell;
use reovim_kernel_interface::{
    Error, ResourceId,
    provider::{Registrar, RegistrarOps, ResourceProvider},
};

use super::{Entry, Registry, access::Scope, codec, registry, reserve};

#[derive(Clone, Copy, Eq, PartialEq)]
enum Phase {
    Inert,
    Starting,
    Fenced,
}

struct Staging {
    registry: Weak<RefCell<Registry>>,
    imports: Option<Weak<RefCell<Scope>>>,
    publication:
        Option<(ResourceId, reovim_kernel_interface::module::ModuleScope)>,
    directory: bool,
    mount: Box<[u8]>,
    required: Vec<Box<[u8]>>,
    entries: Vec<Rc<Entry>>,
    limit: usize,
    non_ascii: bool,
    phase: Phase,
}

/// Runtime retains the batch through Ready publication or completed rollback.
pub(super) struct Batch {
    state: Rc<RefCell<Staging>>,
}

impl Batch {
    pub fn new(
        registry: &Rc<RefCell<Registry>>,
        imports: Option<Weak<RefCell<Scope>>>,
        mount: &[u8],
        required: &[&[u8]],
        limit: usize,
        non_ascii: bool,
    ) -> Result<Self, Error> {
        codec::path(mount, non_ascii)?;
        if required.len() > limit {
            return Err(Error::INVALID);
        }
        let paths = required_paths(mount, required, non_ascii)?;
        let entries = reserve(limit)?;
        Ok(Self {
            state: Rc::new(RefCell::new(Staging {
                registry: Rc::downgrade(registry),
                imports,
                publication: None,
                directory: false,
                mount: Box::from(mount),
                required: paths,
                entries,
                limit,
                non_ascii,
                phase: Phase::Starting,
            })),
        })
    }

    pub fn publication(
        &self,
        namespace: ResourceId,
        scope: reovim_kernel_interface::module::ModuleScope,
    ) {
        self.state.borrow_mut().publication = Some((namespace, scope));
    }

    pub fn directory(&self) {
        self.state.borrow_mut().directory = true;
    }

    pub fn inert(&self) {
        let mut state = self.state.borrow_mut();
        if state.phase != Phase::Fenced {
            state.phase = Phase::Inert;
        }
    }

    pub fn starting(&self) {
        let mut state = self.state.borrow_mut();
        if state.phase == Phase::Inert {
            state.phase = Phase::Starting;
        }
    }

    pub fn fence(&self) {
        self.state.borrow_mut().phase = Phase::Fenced;
    }

    pub fn registrar(&self) -> Registrar {
        Registrar::new(Rc::downgrade(&self.state), RegistrarOps { stage })
    }

    pub fn publish(&self) -> Result<Vec<ResourceId>, Error> {
        let mut state = self.state.borrow_mut();
        if state.phase == Phase::Fenced {
            return Err(Error::CLOSED);
        }
        // Even a failed publication ends staging. Cleanup still owns the batch.
        state.phase = Phase::Fenced;
        let owner = state.registry.upgrade().ok_or(Error::CLOSED)?;
        let mut registry = owner.borrow_mut();
        validate(&state, &registry)?;
        let mut identities = reserve(state.entries.len())?;
        identities.extend(state.entries.iter().map(|entry| entry.identity));
        // All validation, allocation and reference copying precede this cut.
        // Installation only moves retained entries into pre-reserved slots.
        for entry in core::mem::take(&mut state.entries) {
            registry.install(entry);
        }
        Ok(identities)
    }

    pub fn close(&self) -> Result<(), Error> {
        self.state.borrow_mut().phase = Phase::Fenced;
        drain(&self.state)
    }

    /// Direct single-provider admission returns ownership on rejection.
    pub fn take_single(&self) -> ResourceProvider {
        let mut state = self.state.borrow_mut();
        state.phase = Phase::Fenced;
        let entry = state.entries.pop().unwrap();
        if let Some(registry) = state.registry.upgrade() {
            registry.borrow_mut().release();
        }
        drop(state);
        let entry = Rc::try_unwrap(entry).ok().unwrap();
        Rc::try_unwrap(entry.implementation).ok().unwrap().provider.into_inner()
    }
}

impl Drop for Batch {
    fn drop(&mut self) {
        if self.close().is_err() {
            // Abandonment does not prove provider cleanup ended. Keep every
            // remaining staged owner and its code/data obligations alive.
            core::mem::forget(self.state.clone());
        }
    }
}

fn required_paths(
    mount: &[u8],
    required: &[&[u8]],
    non_ascii: bool,
) -> Result<Vec<Box<[u8]>>, Error> {
    let mut paths = reserve(required.len())?;
    for relative in required {
        let path = mounted(mount, relative, non_ascii)?;
        if paths.contains(&path) {
            return Err(Error::CONFLICT);
        }
        paths.push(path);
    }
    Ok(paths)
}

fn drain(owner: &Rc<RefCell<Staging>>) -> Result<(), Error> {
    loop {
        let entry = owner.borrow().entries.last().cloned();
        let Some(entry) = entry else { return Ok(()) };
        entry.implementation.provider.borrow_mut().stop()?;
        let mut state = owner.borrow_mut();
        if let Some(registry) = state.registry.upgrade() {
            registry.borrow_mut().release();
        }
        state.entries.pop();
        drop(state);
        // Final provider destruction runs outside registry and batch guards.
        drop(entry);
    }
}

fn validate(state: &Staging, registry: &Registry) -> Result<(), Error> {
    if registry.closed {
        return Err(Error::CLOSED);
    }
    if state
        .required
        .iter()
        .any(|path| !state.entries.iter().any(|entry| &entry.path == path))
    {
        return Err(Error::UNAVAILABLE);
    }
    for entry in &state.entries {
        if entry.parent
            != super::namespace::parent(&registry.entries, &entry.path)
        {
            return Err(Error::CONFLICT);
        }
        registry.validate(&entry.path, &entry.implementation.interfaces)?;
    }
    Ok(())
}

fn stage(
    owner: &Weak<RefCell<Staging>>,
    relative: &[u8],
    provider: ResourceProvider,
) -> Result<(), (Error, ResourceProvider)> {
    let Some(owner) = owner.upgrade() else {
        return Err((Error::CLOSED, provider));
    };
    let mut state = owner.borrow_mut();
    let Some(registry) = state.registry.upgrade() else {
        return Err((Error::CLOSED, provider));
    };
    let path = match admission(&state, &registry.borrow(), relative, &provider)
    {
        Ok(path) => path,
        Err(error) => return Err((error, provider)),
    };
    let identity = match registry.borrow_mut().reserve() {
        Ok(identity) => identity,
        Err(error) => return Err((error, provider)),
    };
    let parent = super::namespace::parent(&registry.borrow().entries, &path);
    let entry = Rc::new(Entry {
        closed: core::cell::Cell::new(false),
        identity,
        path,
        implementation: crate::engine::Implementation::new(provider),
        imports: state.imports.clone(),
        _origin: None,
        namespace_owner: None,
        directory: state.directory,
        parent,
        publication: state.publication,
    });
    state.entries.push(entry);
    Ok(())
}

fn admission(
    state: &Staging,
    registry: &Registry,
    relative: &[u8],
    provider: &ResourceProvider,
) -> Result<Box<[u8]>, Error> {
    if state.phase == Phase::Fenced {
        return Err(Error::CLOSED);
    }
    if state.phase == Phase::Inert {
        return Err(Error::NOT_READY);
    }
    if state.entries.len() == state.limit {
        return Err(Error::PRESSURE);
    }
    let path = mounted(&state.mount, relative, state.non_ascii)?;
    registry.validate(&path, provider.interfaces())?;
    for entry in &state.entries {
        registry::compatible(entry, &path, provider.interfaces())?;
    }
    Ok(path)
}

fn mounted(
    mount: &[u8],
    relative: &[u8],
    non_ascii: bool,
) -> Result<Box<[u8]>, Error> {
    if relative.starts_with(b"/") {
        return Err(Error::INVALID);
    }
    let separator = usize::from(mount != b"/" && !relative.is_empty());
    let length = mount
        .len()
        .checked_add(relative.len())
        .and_then(|size| size.checked_add(separator))
        .ok_or(Error::PRESSURE)?;
    if length > 4096 {
        return Err(Error::PRESSURE);
    }
    let mut path = reserve(length)?;
    path.extend_from_slice(mount);
    if separator != 0 {
        path.push(b'/');
    }
    path.extend_from_slice(relative);
    codec::path(&path, non_ascii)?;
    Ok(path.into_boxed_slice())
}

#[cfg(test)]
#[path = "activation/tests.rs"]
mod tests;
