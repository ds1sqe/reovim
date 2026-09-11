use alloc::{boxed::Box, rc::Rc, vec::Vec};
use core::cell::RefCell;
use reovim_kernel_interface::access::{Access, Method, Reply};
use reovim_kernel_interface::namespace::ResourceEntry;
use reovim_kernel_interface::provider::Permissions;
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, RequestError, ResourceId,
};

use crate::namespace::{Entry, Kind, Scope, validate};
use crate::request::{Record, Wait, Work};
use crate::{Shared, diagnostics::Stage};

struct Caller {
    shared: Rc<RefCell<Shared>>,
    scope: Rc<RefCell<Scope>>,
    generation: u64,
}

pub fn connection(
    shared: Rc<RefCell<Shared>>,
    scope: Rc<RefCell<Scope>>,
    generation: u64,
) -> Connection {
    Connection::new(Rc::new(Caller { shared, scope, generation }))
}

impl Access for Caller {
    fn root(&self) -> Result<ResourceId, Error> {
        self.check()?;
        self.scope.borrow_mut().export(&mut self.shared.borrow_mut(), 1)
    }

    fn resolve_at(
        &self,
        parent: ResourceId,
        path: &str,
    ) -> Result<ResourceId, Error> {
        self.check()?;
        let mut shared = self.shared.borrow_mut();
        let mut scope = self.scope.borrow_mut();
        let mut key = scope.key(parent)?;
        validate(path, shared.limits.path_bytes)?;
        if !scope.visible(&shared, key) {
            return Err(Error::DENIED);
        }
        for name in path.split('/').filter(|name| !name.is_empty()) {
            key = shared.path(key, name)?;
            scope.require_visible(&shared, key)?;
        }
        scope.export(&mut shared, key)
    }

    fn children(
        &self,
        parent: ResourceId,
    ) -> Result<Vec<ResourceEntry>, Error> {
        self.check()?;
        let mut shared = self.shared.borrow_mut();
        let mut scope = self.scope.borrow_mut();
        let key = scope.key(parent)?;
        if !scope.visible(&shared, key) {
            return Err(Error::DENIED);
        }
        let kind = &shared.entries[shared.entry_index(key)?].kind;
        if !matches!(kind, Kind::Directory | Kind::Buffer(_)) {
            return Err(Error::UNSUPPORTED);
        }
        let entries: Vec<_> = shared
            .entries
            .iter()
            .filter(|entry| {
                entry.parent == key
                    && entry.key != key
                    && scope.visible(&shared, entry.key)
            })
            .map(|entry| (entry.name.clone(), entry.key))
            .collect();
        let mut result = Vec::new();
        result.try_reserve_exact(entries.len()).map_err(|_| Error::PRESSURE)?;
        for (name, key) in entries {
            result.push(ResourceEntry {
                name,
                resource: scope.export(&mut shared, key)?,
            });
        }
        result.sort_unstable_by(|a, b| a.name.cmp(&b.name));
        Ok(result)
    }

    fn request(
        &self,
        method: Method,
        subject: ResourceId,
        input: &Message,
    ) -> Reply {
        match self.admit(method, subject, input) {
            Ok(reply) => reply,
            Err(error) => Box::pin(core::future::ready(Err(
                RequestError::Rejected(error),
            ))),
        }
    }

    fn close(&self, access: ResourceId) -> Result<(), Error> {
        self.check()?;
        let mut shared = self.shared.borrow_mut();
        let key = self.scope.borrow().key(access)?;
        let kind = shared.entries[shared.entry_index(key)?].kind.clone();
        if let Kind::Work { scope, request } = kind {
            let retired = self.close_work(&mut shared, scope, request)?;
            drop(shared);
            drop(retired);
            return Ok(());
        }
        let scope_id = self.scope.borrow().id;
        if shared
            .requests
            .iter()
            .any(|record| record.borrow().scope.borrow().id == scope_id)
        {
            return Err(Error::BUSY);
        }
        self.scope
            .borrow_mut()
            .grants
            .retain(|(grant, _)| !shared.descends(*grant, key));
        Ok(())
    }

    fn reconnect(&self) -> Result<Connection, Error> {
        self.check()?;
        let mut scope = self.scope.borrow_mut();
        scope.generation =
            scope.generation.checked_add(1).ok_or(Error::PRESSURE)?;
        scope.names.clear();
        Ok(connection(
            Rc::clone(&self.shared),
            Rc::clone(&self.scope),
            scope.generation,
        ))
    }
}

impl Caller {
    fn check(&self) -> Result<(), Error> {
        if self.scope.borrow().generation != self.generation {
            return Err(Error::STALE);
        }
        let shared = self.shared.borrow();
        if shared.entries.is_empty() {
            return Err(Error::CLOSED);
        }
        if !shared.ready {
            return Err(Error::NOT_READY);
        }
        Ok(())
    }

    fn admit(
        &self,
        method: Method,
        subject: ResourceId,
        input: &Message,
    ) -> Result<Reply, Error> {
        self.check()?;
        let mut shared = self.shared.borrow_mut();
        let scope = self.scope.borrow();
        let key = scope.key(subject)?;
        let target = shared.entries[shared.entry_index(key)?].kind.clone();
        let owner = scope.id;
        if matches!(target, Kind::Work { scope, .. } if scope != owner) {
            return Err(Error::DENIED);
        }
        if let Kind::Work { request, .. } = target {
            return self.recovery(&shared, request, method, input);
        }
        if shared.closing {
            return Err(Error::CLOSED);
        }
        if !scope.permissions(&shared, key).is_some_and(|p| p.allows(method)) {
            return Err(Error::DENIED);
        }
        let output = validate_contract(&target, method, input)?;
        Self::check_capacity(&shared, input)?;
        let mut captured = input.clone();
        translate_input(&shared, &scope, &target, method, &mut captured)?;
        let id = shared.id()?;
        let resource = shared.id()?;
        let recovery = shared.work_path(scope.id, id);
        let name =
            recovery.strip_prefix("requests/").ok_or(Error::INVALID)?.into();
        let parent = shared.active;
        let record = Rc::new(RefCell::new(Record {
            id,
            parent,
            scope: Rc::clone(&self.scope),
            subject: key,
            method,
            input: captured,
            work: Some(Work::Queued),
            outcome: None,
            target,
            output,
            cancelled: false,
            observed: false,
            recovery,
            waker: None,
        }));
        shared.entries.push(Entry {
            key: resource,
            parent: 2,
            name,
            kind: Kind::Work { scope: scope.id, request: id },
        });
        shared.requests.push(Rc::clone(&record));
        shared.emit(id, scope.id, key, Stage::Admitted, parent);
        Ok(Box::pin(Wait {
            shared: Rc::clone(&self.shared),
            record: Some(record),
            recovery: false,
            generation: self.generation,
        }))
    }

    fn check_capacity(shared: &Shared, input: &Message) -> Result<(), Error> {
        if shared.requests.len() == shared.limits.requests
            || shared.entries.len() == shared.limits.resources
            || input.data.len() > shared.limits.message_bytes
            || input.contract.name.len() > shared.limits.path_bytes
            || input.resources.len() > shared.limits.resources
            || !shared.ids.can_take(2)
        {
            return Err(Error::PRESSURE);
        }
        Ok(())
    }

    fn recovery(
        &self,
        shared: &Shared,
        id: u64,
        method: Method,
        input: &Message,
    ) -> Result<Reply, Error> {
        if !input.data.is_empty() || !input.resources.is_empty() {
            return Err(Error::INVALID);
        }
        let record = shared
            .requests
            .iter()
            .find(|record| record.borrow().id == id)
            .cloned()
            .ok_or(Error::UNAVAILABLE)?;
        let status = if record.borrow().outcome.is_some() {
            b"finished".as_slice()
        } else {
            b"pending".as_slice()
        };
        let reply = match (
            method,
            input.contract.name.as_ref(),
            input.contract.version,
        ) {
            (Method::Read, "kernel.request.result", 1) => {
                return Ok(Box::pin(Wait {
                    shared: Rc::clone(&self.shared),
                    record: Some(record),
                    recovery: true,
                    generation: self.generation,
                }));
            }
            (Method::Write, "kernel.request.cancel", 1) => {
                record.borrow_mut().cancelled = true;
                Message::new(input.contract.clone(), b"requested")
            }
            (Method::Read, "kernel.request.status", 1) => {
                Message::new(input.contract.clone(), status)
            }
            _ => return Err(Error::UNSUPPORTED),
        };
        Ok(Box::pin(core::future::ready(Ok(reply))))
    }

    fn close_work(
        &self,
        shared: &mut Shared,
        owner: u64,
        request: u64,
    ) -> Result<Option<Rc<RefCell<Record>>>, Error> {
        if owner != self.scope.borrow().id {
            return Err(Error::DENIED);
        }
        let record = shared
            .requests
            .iter()
            .find(|record| record.borrow().id == request)
            .ok_or(Error::UNAVAILABLE)?;
        if record.borrow().outcome.is_none() {
            return Err(Error::BUSY);
        }
        Ok(shared.retire(request))
    }
}

fn validate_contract(
    target: &Kind,
    method: Method,
    input: &Message,
) -> Result<Box<[Permissions]>, Error> {
    match target {
        Kind::Provider(provider) => provider
            .borrow()
            .surfaces
            .iter()
            .find(|surface| {
                surface.contract == input.contract
                    && surface.methods.allows(method)
            })
            .map(|surface| surface.output.clone())
            .ok_or(Error::UNSUPPORTED),
        Kind::Model(model)
            if method == Method::Write
                && input.contract == model.borrow().create =>
        {
            Ok(Box::new([Permissions::ALL, Permissions::ALL]))
        }
        Kind::Buffer(_) | Kind::ClosedBuffer { .. }
            if method == Method::Write
                && crate::closing::contract(&input.contract) =>
        {
            Ok(Box::new([]))
        }
        Kind::Buffer(buffer) => buffer_contract(buffer, method, input),
        Kind::History(_) if history_contract(method, &input.contract) => {
            Ok(Box::new([]))
        }
        _ => Err(Error::UNSUPPORTED),
    }
}

fn buffer_contract(
    buffer: &Rc<RefCell<crate::Editing>>,
    method: Method,
    input: &Message,
) -> Result<Box<[Permissions]>, Error> {
    let buffer = buffer.borrow();
    let model = buffer.model.borrow();
    let contract =
        if method == Method::Read { &model.read } else { &model.edit };
    if &input.contract != contract {
        return Err(Error::UNSUPPORTED);
    }
    Ok(Box::new([Permissions::READ, Permissions::READ]))
}

fn history_contract(method: Method, contract: &Contract) -> bool {
    contract.version == 1
        && matches!(
            (method, contract.name.as_ref()),
            (Method::Read, "kernel.history.children")
                | (
                    Method::Write,
                    "kernel.history.undo" | "kernel.history.redo"
                )
        )
}

fn translate_input(
    shared: &Shared,
    scope: &Scope,
    target: &Kind,
    method: Method,
    input: &mut Message,
) -> Result<(), Error> {
    if matches!(target, Kind::ClosedBuffer { .. }) {
        if input.resources.len() != 1 {
            return Err(Error::INVALID);
        }
        input.resources[0] = ResourceId(scope.key(input.resources[0])?);
        return Ok(());
    }
    if let Kind::Buffer(buffer) | Kind::History(buffer) = target {
        if method == Method::Read && !input.resources.is_empty() {
            return Err(Error::INVALID);
        }
        if method == Method::Read {
            return Ok(());
        }
        if input.resources.len() != 1 {
            return Err(Error::INVALID);
        }
        let key = scope.key(input.resources[0])?;
        if key != buffer.borrow().resource {
            return Err(Error::STALE);
        }
        input.resources[0] = ResourceId(key);
        return Ok(());
    }
    let Kind::Provider(provider) = target else {
        return if input.resources.is_empty() {
            Ok(())
        } else {
            Err(Error::INVALID)
        };
    };
    let provider = provider.borrow();
    let surface = provider
        .surfaces
        .iter()
        .find(|s| s.contract == input.contract && s.methods.allows(method))
        .ok_or(Error::UNSUPPORTED)?;
    if surface.input.len() != input.resources.len() {
        return Err(Error::INVALID);
    }
    for (reference, required) in input.resources.iter_mut().zip(&surface.input)
    {
        let key = scope.key(*reference)?;
        if !scope
            .permissions(shared, key)
            .is_some_and(|rights| rights.contains(*required))
        {
            return Err(Error::DENIED);
        }
        *reference = ResourceId(key);
    }
    Ok(())
}
