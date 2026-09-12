//! Composition binds providers to the namespace used by their imports.

use alloc::{rc::Rc, vec::Vec};
use core::cell::RefCell;
use reovim_kernel_interface::{
    Connection, Error, Message, ResourceId,
    module::Module,
    provider::{Contribution, Permissions},
};

use crate::{Runtime, Shared, namespace::Scope, request::Record};

/// An assembly-owned provider binding. Modules receive only its Connection.
/// Calls translate reference slots into this exact receiving namespace.
#[derive(Clone)]
pub struct Binding {
    pub(crate) shared: Rc<RefCell<Shared>>,
    pub(crate) scope: Rc<RefCell<Scope>>,
}

impl Binding {
    /// Resolves an assembly identity through this binding's granted namespace.
    /// Modules receive Connection identities instead of these registry keys.
    /// # Errors
    /// Rejects closed bindings, invalid paths and inaccessible resources.
    pub fn resource(&self, path: &str) -> Result<ResourceId, Error> {
        let shared = self.shared.borrow();
        let scope = self.scope.borrow();
        if scope.closed {
            return Err(Error::CLOSED);
        }
        crate::namespace::validate(path, shared.limits.path_bytes)?;
        let key = path
            .split('/')
            .filter(|name| !name.is_empty())
            .try_fold(scope.root, |parent, name| {
                scope.child(&shared, parent, name)
            })?;
        scope.require_visible(&shared, key)?;
        Ok(ResourceId(key))
    }

    /// Resolves a publication name beneath this namespace's owned root.
    /// An alias names another owner's subtree and cannot receive publications.
    /// # Errors
    /// Rejects invalid names, alias traversal and a removed namespace root.
    pub fn publication_path(
        &self,
        path: &str,
    ) -> Result<alloc::boxed::Box<str>, Error> {
        let shared = self.shared.borrow();
        crate::namespace::validate(path, shared.limits.path_bytes)?;
        let scope = self.scope.borrow();
        if scope
            .aliases
            .borrow()
            .iter()
            .any(|(name, _)| Some(&**name) == path.split('/').next())
        {
            return Err(Error::DENIED);
        }
        publication_path(&shared, scope.root, path)
    }

    #[must_use]
    pub fn connection(&self) -> Connection {
        crate::access::connection(
            self.shared.clone(),
            self.scope.clone(),
            self.scope.borrow().generation,
        )
    }
}

fn publication_path(
    shared: &Shared,
    mut root: u64,
    path: &str,
) -> Result<alloc::boxed::Box<str>, Error> {
    let mut parts = Vec::new();
    for _ in 0..shared.entries.len() {
        if root == 1 {
            parts.reverse();
            parts.extend((!path.is_empty()).then_some(path));
            return Ok(parts.join("/").into());
        }
        let entry = &shared.entries[shared.entry_index(root)?];
        parts.push(&entry.name);
        root = entry.parent;
    }
    Err(Error::INVALID)
}

impl Runtime {
    /// Fences the selected client's published owned accesses and wakes their
    /// pending calls. Other clients and shared subjects remain alive.
    ///
    /// This starts access cleanup, not connection revocation: admitted results
    /// and their recovery paths remain usable until the assembly adopts them.
    /// # Errors
    /// Rejects a binding belonging to another Runtime.
    pub fn close_owned_access(
        &mut self,
        client: &Binding,
    ) -> Result<(), Error> {
        if !Rc::ptr_eq(&self.shared, &client.shared) {
            return Err(Error::INVALID);
        }
        let owner = client.scope.borrow().owner;
        crate::owned::close_owner(&self.shared, owner)
    }

    /// Creates a separate import namespace under the retained client's owner.
    /// Grants use assembly Resource identities and cannot exceed the client's
    /// authority. Sharing an owner does not share names or request access.
    /// # Errors
    /// Rejects foreign bindings, excess grants or exhausted caller capacity.
    pub fn bind_for(
        &mut self,
        client: &Binding,
        grants: &[(ResourceId, Permissions)],
    ) -> Result<Binding, Error> {
        if !Rc::ptr_eq(&self.shared, &client.shared) {
            return Err(Error::INVALID);
        }
        let shared = self.shared.borrow();
        let scope = client.scope.borrow();
        if scope.closed {
            return Err(Error::CLOSED);
        }
        if !grants.iter().all(|(resource, rights)| {
            scope
                .permissions(&shared, resource.0)
                .is_some_and(|allowed| allowed.contains(*rights))
        }) {
            return Err(Error::DENIED);
        }
        let owner = scope.owner;
        let root = scope.root;
        let aliases = scope.aliases.clone();
        drop(scope);
        drop(shared);
        let binding = self.bind(grants)?;
        binding.scope.borrow_mut().owner = owner;
        binding.scope.borrow_mut().root = root;
        binding.scope.borrow_mut().aliases = aliases;
        Ok(binding)
    }

    /// Selects a client's namespace root independently of operation grants.
    /// Recovery keeps its ordinary requests path within this root.
    /// # Errors
    /// Rejects an invalid root, occupied recovery name or caller capacity.
    pub fn bind_at(
        &mut self,
        root: ResourceId,
        grants: &[(ResourceId, Permissions)],
    ) -> Result<Binding, Error> {
        let shared = self.shared.borrow();
        let index = shared.entry_index(root.0)?;
        if !shared.entries[index].kind.accepts_children()
            || (root.0 != 1 && shared.path(root.0, "requests").is_ok())
        {
            return Err(Error::INVALID);
        }
        drop(shared);
        let binding = self.bind(grants)?;
        binding.scope.borrow_mut().root = root.0;
        if root.0 != 1 {
            self.mount(&binding, "requests", ResourceId(2))?;
        }
        Ok(binding)
    }

    /// Adds a root-level name for a shared subject; this grants no operations.
    /// Bound module namespaces see the same names under their own permissions.
    /// # Errors
    /// Rejects foreign bindings, duplicate names, invalid targets or capacity.
    pub fn mount(
        &mut self,
        binding: &Binding,
        name: &str,
        target: ResourceId,
    ) -> Result<(), Error> {
        if !Rc::ptr_eq(&self.shared, &binding.shared) {
            return Err(Error::INVALID);
        }
        let shared = self.shared.borrow();
        if shared.closing {
            return Err(Error::CLOSED);
        }
        shared.entry_index(target.0)?;
        crate::namespace::validate(name, shared.limits.path_bytes)?;
        let scope = binding.scope.borrow();
        if scope.closed {
            return Err(Error::CLOSED);
        }
        shared.entry_index(scope.root)?;
        if name.is_empty()
            || name.contains('/')
            || shared.path(scope.root, name).is_ok()
            || scope.aliases.borrow().iter().any(|(alias, _)| &**alias == name)
        {
            return Err(Error::INVALID);
        }
        let mut aliases = scope.aliases.borrow_mut();
        if aliases.len() == shared.limits.resources {
            return Err(Error::PRESSURE);
        }
        aliases.try_reserve(1).map_err(|_| Error::PRESSURE)?;
        aliases.push((name.into(), target.0));
        Ok(())
    }

    /// Creates a provider's explicit import namespace for composition.
    /// # Errors
    /// Rejects unknown grants, closing state or exhausted caller capacity.
    pub fn bind(
        &mut self,
        grants: &[(ResourceId, Permissions)],
    ) -> Result<Binding, Error> {
        let mut shared = self.shared.borrow_mut();
        shared.scopes.retain(|scope| scope.strong_count() != 0);
        if shared.closing || shared.scopes.len() == shared.limits.callers {
            return Err(Error::PRESSURE);
        }
        for (resource, _) in grants {
            shared.entry_index(resource.0)?;
        }
        let scope = Rc::new(RefCell::new(Scope::new(shared.id()?, grants)));
        shared.scopes.push(Rc::downgrade(&scope));
        Ok(Binding { shared: self.shared.clone(), scope })
    }

    /// Publishes a group whose reference slots use the bound import namespace.
    /// # Errors
    /// Rejection returns every contribution; a foreign binding is invalid.
    pub fn register_bound(
        &mut self,
        binding: &Binding,
        values: Vec<Contribution>,
    ) -> Result<Vec<ResourceId>, (Error, Vec<Contribution>)> {
        if !Rc::ptr_eq(&self.shared, &binding.shared) {
            return Err((Error::INVALID, values));
        }
        if binding.scope.borrow().closed {
            return Err((Error::CLOSED, values));
        }
        let ids = self.register_contributions(values)?;
        attach(&mut self.shared.borrow_mut(), binding, &ids);
        Ok(ids)
    }

    /// Activates an inert module with its actual provider import namespace.
    /// # Errors
    /// Rejection returns the module without publishing contributions.
    pub fn activate_bound_module(
        &mut self,
        binding: &Binding,
        path: &str,
        module: Module,
    ) -> Result<ResourceId, (Error, Module)> {
        if !Rc::ptr_eq(&self.shared, &binding.shared) {
            return Err((Error::INVALID, module));
        }
        if binding.scope.borrow().closed {
            return Err((Error::CLOSED, module));
        }
        let id = self.activate_module(path, module)?;
        self.attach_binding(binding);
        Ok(id)
    }

    pub(super) fn attach_binding(&mut self, binding: &Binding) {
        self.modules.last_mut().expect("adopted module").binding =
            Some(binding.clone());
    }
}

fn attach(shared: &mut Shared, binding: &Binding, ids: &[ResourceId]) {
    for id in ids {
        let index = shared.entry_index(id.0).expect("new contribution");
        let entry = &mut shared.entries[index];
        entry.receiver = matches!(
            entry.kind,
            crate::namespace::Kind::Provider(_)
                | crate::namespace::Kind::Model(_)
        )
        .then(|| binding.scope.clone());
    }
}

pub fn receiver(
    shared: &Shared,
    key: u64,
    input: &Message,
) -> Result<Option<Rc<RefCell<Scope>>>, Error> {
    let entry = &shared.entries[shared.entry_index(key)?];
    if matches!(entry.kind, crate::namespace::Kind::Model(_))
        && !input.resources.is_empty()
        && entry.receiver.is_none()
    {
        return Err(Error::INVALID);
    }
    Ok(entry.receiver.clone())
}

pub fn capture(
    shared: &Shared,
    input: &Message,
) -> Result<Vec<crate::namespace::Kind>, Error> {
    input
        .resources
        .iter()
        .map(|id| {
            shared
                .entry_index(id.0)
                .map(|index| shared.entries[index].kind.clone())
        })
        .collect()
}

pub fn receive(
    shared: &mut Shared,
    record: &Record,
    mut input: Message,
) -> Result<(ResourceId, Message), Error> {
    let Some(receiver) = &record.receiver else {
        if forwarding(record) {
            return Err(Error::INVALID);
        }
        return Ok((ResourceId(record.subject), input));
    };
    let mut receiver = receiver.borrow_mut();
    let rights = input_rights(shared, record)?;
    if receiver.loans.len() + rights.len() > shared.limits.resources {
        return Err(Error::PRESSURE);
    }
    receiver.loans.try_reserve(rights.len()).map_err(|_| Error::PRESSURE)?;
    let subject = receiver.export(shared, record.subject)?;
    for (id, rights) in input.resources.iter_mut().zip(rights) {
        let key = id.0;
        *id = receiver.export(shared, key)?;
        receiver.loans.push((record.id, key, rights));
    }
    Ok((subject, input))
}

fn input_rights(
    shared: &Shared,
    record: &Record,
) -> Result<Vec<Permissions>, Error> {
    if let crate::namespace::Kind::Model(model) = &record.target {
        let model = model.borrow();
        return Ok(if record.input.contract == model.create {
            model.initial_input.to_vec()
        } else {
            Vec::new()
        });
    }
    if forwarding(record) {
        let scope = record.scope.borrow();
        return record
            .input
            .resources
            .iter()
            .map(|id| scope.permissions(shared, id.0).ok_or(Error::DENIED))
            .collect();
    }
    let crate::namespace::Kind::Provider(provider) = &record.target else {
        return Err(Error::INVALID);
    };
    provider
        .borrow()
        .surfaces
        .iter()
        .find(|s| {
            s.contract == record.input.contract
                && s.methods.allows(record.method)
        })
        .map(|s| s.input.to_vec())
        .ok_or(Error::UNSUPPORTED)
}

pub fn forwarding(record: &Record) -> bool {
    match &record.target {
        crate::namespace::Kind::Provider(provider) => {
            provider.borrow().forwarding
        }
        _ => false,
    }
}

pub fn output_rights(
    shared: &Shared,
    record: &Record,
    message: &Message,
) -> Result<alloc::boxed::Box<[Permissions]>, Error> {
    let receiver = record.receiver.as_ref().ok_or(Error::PROVIDER)?.borrow();
    message
        .resources
        .iter()
        .map(|id| {
            receiver
                .permissions(shared, receiver.key(*id)?)
                .ok_or(Error::PROVIDER)
        })
        .collect()
}

pub fn error(record: &Record, mut error: Error) -> Error {
    if let Some(receiver) = &record.receiver {
        error.id = error
            .id
            .and_then(|id| receiver.borrow().key(id).ok())
            .map(ResourceId);
    }
    error
}

pub fn returned(
    shared: &Shared,
    record: &Record,
    message: &mut Message,
) -> Result<(), Error> {
    // Model creation replies contain Runtime-owned Buffer/History identities.
    if matches!(record.target, crate::namespace::Kind::Model(_)) {
        return Ok(());
    }
    let Some(receiver) = &record.receiver else { return Ok(()) };
    let receiver = receiver.borrow();
    for (resource, rights) in message.resources.iter_mut().zip(&record.output) {
        let key = receiver.key(*resource)?;
        if !receiver
            .permissions(shared, key)
            .is_some_and(|allowed| allowed.contains(*rights))
        {
            return Err(Error::PROVIDER);
        }
        *resource = ResourceId(key);
    }
    drop(receiver);
    grant_outputs(
        shared,
        &mut record.scope.borrow_mut(),
        &record.output,
        message,
    )
}

pub fn grant_outputs(
    shared: &Shared,
    caller: &mut Scope,
    output: &[Permissions],
    message: &Message,
) -> Result<(), Error> {
    caller.grants.try_reserve(output.len()).map_err(|_| Error::PRESSURE)?;
    for (resource, rights) in message.resources.iter().zip(output) {
        if !caller
            .permissions(shared, resource.0)
            .is_some_and(|allowed| allowed.contains(*rights))
        {
            caller.grants.push((resource.0, *rights));
        }
    }
    Ok(())
}

/// Retain the authorized work identity before exposing lost-reply evidence.
pub fn capture_recovery(
    shared: &mut Shared,
    record: &Record,
    locator: &str,
) -> Option<(u64, Permissions)> {
    let receiver = record.receiver.as_ref()?;
    let key = shared.path(1, locator).ok()?;
    let rights = receiver.borrow().permissions(shared, key)?;
    let index = shared.entry_index(key).ok()?;
    let crate::namespace::Kind::Work { retained, .. } =
        &mut shared.entries[index].kind
    else {
        return None;
    };
    *retained = true;
    Some((key, rights))
}

/// Opaque external evidence is preserved without inventing local authority.
pub fn recovery(
    shared: &Shared,
    record: &Record,
    caller: &mut Scope,
) -> Result<(), Error> {
    let Some((key, rights)) = record.recovery_work else { return Ok(()) };
    if !caller
        .permissions(shared, key)
        .is_some_and(|allowed| allowed.contains(rights))
    {
        caller.grants.try_reserve(1).map_err(|_| Error::PRESSURE)?;
        caller.grants.push((key, rights));
    }
    Ok(())
}

/// Reserve a selected namespace's live-state directory and its new subjects.
pub fn state_container(
    shared: &Shared,
    scope: &Scope,
    name: &str,
    subjects: usize,
) -> Result<Option<u64>, Error> {
    let parent = match scope.child(shared, scope.root, name) {
        Ok(key) => Some(key),
        Err(Error::UNAVAILABLE) => None,
        Err(error) => return Err(error),
    };
    if let Some(key) = parent {
        let index = shared.entry_index(key)?;
        if !matches!(
            shared.entries[index].kind,
            crate::namespace::Kind::Directory
        ) {
            return Err(Error::INVALID);
        }
    }
    let needed = subjects.saturating_add(usize::from(parent.is_none()));
    if shared.entries.len().saturating_add(needed) > shared.limits.resources
        || !shared.ids.can_take(needed)
    {
        return Err(Error::PRESSURE);
    }
    Ok(parent)
}
