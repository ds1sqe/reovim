use alloc::{boxed::Box, rc::Rc, vec::Vec};
use core::cell::{Cell, RefCell};
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
    capture: Option<Rc<crate::namespace::Capture>>,
    view: Option<Rc<Cell<bool>>>,
}

pub fn connection(
    shared: Rc<RefCell<Shared>>,
    scope: Rc<RefCell<Scope>>,
    generation: u64,
) -> Connection {
    Connection::new(Rc::new(Caller {
        shared,
        scope,
        generation,
        capture: None,
        view: None,
    }))
}

pub fn view_connection(
    shared: Rc<RefCell<Shared>>,
    scope: Rc<RefCell<Scope>>,
    view: Rc<Cell<bool>>,
) -> Connection {
    Connection::new(Rc::new(Caller {
        shared,
        scope,
        generation: 0,
        capture: None,
        view: Some(view),
    }))
}

impl Access for Caller {
    fn read_view(&self, subject: ResourceId) -> Result<Connection, Error> {
        self.check()?;
        crate::read_views::acquire(&self.shared, &self.scope, subject)
    }
    fn capture(
        &self,
        subject: ResourceId,
        rights: Permissions,
    ) -> Result<Connection, Error> {
        self.check()?;
        let shared = self.shared.borrow();
        if shared.closing {
            return Err(Error::CLOSED);
        }
        let mut scope = self.scope.borrow_mut();
        let key = scope.key(subject)?;
        if rights.is_empty()
            || !scope
                .permissions(&shared, key)
                .is_some_and(|allowed| allowed.contains(rights))
        {
            return Err(Error::DENIED);
        }
        let entry = &shared.entries[shared.entry_index(key)?];
        if entry.closed()
            || matches!(&entry.kind, Kind::Participation(p)
                if p.borrow().settled())
            || matches!(
                entry.kind,
                Kind::ClosedBuffer { .. } | Kind::ClosedSession { .. }
            )
        {
            return Err(Error::CLOSED);
        }
        crate::modules::require_ready(entry.activation.as_deref())?;
        scope.captures.retain(|capture| capture.strong_count() != 0);
        if scope.captures.len() == shared.limits.resources {
            return Err(Error::PRESSURE);
        }
        scope.captures.try_reserve(1).map_err(|_| Error::PRESSURE)?;
        let capture = Rc::new(crate::namespace::Capture {
            key,
            rights,
            _target: entry.kind.clone(),
            _view: match &entry.kind {
                Kind::ReadView { live, .. } => live.upgrade(),
                _ => None,
            },
        });
        scope.captures.push(Rc::downgrade(&capture));
        Ok(Connection::new(Rc::new(Self {
            shared: self.shared.clone(),
            scope: self.scope.clone(),
            generation: self.generation,
            capture: Some(capture),
            view: self.view.clone(),
        })))
    }
    fn withdraw(&self, resource: ResourceId) -> Result<(), Error> {
        self.check()?;
        crate::publication::withdraw(&self.shared, &self.scope, resource)
    }

    fn publish(
        &self,
        parent: ResourceId,
        name: &str,
        provider: reovim_kernel_interface::provider::ResourceProvider,
    ) -> Result<
        ResourceId,
        (Error, reovim_kernel_interface::provider::ResourceProvider),
    > {
        if let Err(error) = self.check() {
            return Err((error, provider));
        }
        crate::publication::publish(
            &self.shared,
            &self.scope,
            parent,
            name,
            provider,
        )
    }

    fn root(&self) -> Result<ResourceId, Error> {
        self.check()?;
        let root = self.scope.borrow().root;
        self.scope.borrow_mut().export(&mut self.shared.borrow_mut(), root)
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
            key = scope.child(&shared, key, name)?;
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
        if !matches!(
            kind,
            Kind::Directory
                | Kind::Provider(_)
                | Kind::Model(_)
                | Kind::Buffer(_)
                | Kind::Session(_)
                | Kind::ClosedSession { .. }
        ) {
            return Err(Error::UNSUPPORTED);
        }
        let entries = scope.children(&shared, key);
        if entries.len() > shared.limits.resources {
            return Err(Error::PRESSURE);
        }
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
        if let Some(view) =
            self.view.as_ref().filter(|_| key == self.scope.borrow().root)
        {
            let scope = self.scope.borrow().id;
            return crate::read_views::close(&shared, scope, view);
        }
        if shared.entries[shared.entry_index(key)?].ownership.is_some() {
            drop(shared);
            return crate::owned::close(&self.shared, &self.scope, key);
        }
        let kind = shared.entries[shared.entry_index(key)?].kind.clone();
        if let Kind::Work { request, .. } = kind {
            let retired = self.close_work(&mut shared, key, request)?;
            drop(shared);
            drop(retired);
            return Ok(());
        }
        // A borrowed subject is not an owned access. Revoking this import's
        // grants would not close the caller's access or the shared subject.
        Err(Error::DENIED)
    }

    fn reconnect(&self) -> Result<Connection, Error> {
        self.check()?;
        let mut scope = self.scope.borrow_mut();
        scope.generation =
            scope.generation.checked_add(1).ok_or(Error::PRESSURE)?;
        scope.names.clear();
        Ok(Connection::new(Rc::new(Self {
            shared: self.shared.clone(),
            scope: self.scope.clone(),
            generation: scope.generation,
            capture: self.capture.clone(),
            view: self.view.clone(),
        })))
    }
}

impl Caller {
    fn check(&self) -> Result<(), Error> {
        if self.view.as_ref().is_some_and(|view| !view.get()) {
            return Err(Error::CLOSED);
        }
        if self.scope.borrow().closed {
            return Err(Error::CLOSED);
        }
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
        if shared.entries[shared.entry_index(key)?].closed() {
            return Err(Error::CLOSED);
        }
        let target = match &shared.entries[shared.entry_index(key)?].kind {
            Kind::ReadView { provider, .. } => Kind::Provider(provider.clone()),
            kind => kind.clone(),
        };
        let rights = scope
            .permissions(&shared, key)
            .filter(|rights| rights.allows(method))
            .ok_or(Error::DENIED)?;
        if let Kind::Work { request, .. } = target {
            return self.recovery(&mut shared, request, method, input);
        }
        if shared.closing
            && !(matches!(target, Kind::Module(_))
                && crate::modules::during_close(method, &input.contract))
        {
            return Err(Error::CLOSED);
        }
        let activation =
            shared.entries[shared.entry_index(key)?].activation.clone();
        let closing_buffer =
            matches!(target, Kind::Buffer(_) | Kind::ClosedBuffer { .. })
                && method == Method::Write
                && crate::closing::contract(&input.contract);
        let closing_session =
            matches!(target, Kind::Session(_) | Kind::ClosedSession { .. })
                && method == Method::Write
                && crate::sessions::closing(&input.contract);
        let activation =
            if closing_buffer || closing_session { None } else { activation };
        crate::modules::require_ready(activation.as_deref())?;
        let output = validate_contract(&target, method, input)?;
        Self::check_capacity(&shared, input)?;
        let mut captured = input.clone();
        let snapshot = crate::sessions::capture_snapshot(
            &shared,
            &scope,
            &target,
            method,
            key,
            &mut captured,
        )?;
        if snapshot.is_none() {
            translate_input(&shared, &scope, &target, method, &mut captured)?;
        }
        let id = shared.id()?;
        let references = crate::binding::capture(&shared, &captured)?;
        let receiver_key =
            if snapshot.is_some() { captured.resources[0].0 } else { key };
        let receiver =
            crate::binding::receiver(&shared, receiver_key, &captured)?;
        let activation = if receiver_key == key {
            activation
        } else {
            shared.entries[shared.entry_index(receiver_key)?].activation.clone()
        };
        crate::modules::require_ready(activation.as_deref())?;
        let resource = shared.id()?;
        let recovery = shared.work_path(scope.id, id);
        let name =
            recovery.strip_prefix("requests/").ok_or(Error::INVALID)?.into();
        let parent = shared.active;
        let record = Rc::new(RefCell::new(Record {
            _capture: self.capture.clone(),
            id,
            parent,
            scope: Rc::clone(&self.scope),
            receiver,
            subject: key,
            method,
            rights,
            input: captured,
            work: Some(Work::Queued),
            outcome: None,
            target,
            references,
            activation,
            output,
            snapshot,
            backing: None,
            cancelled: false,
            observed: false,
            readers: 0,
            recovery_work: None,
            recovery,
            wakers: Vec::new(),
            schedule: crate::request::Schedule::new(),
        }));
        Ok(self.publish_request(&mut shared, record, resource, name))
    }

    fn publish_request(
        &self,
        shared: &mut Shared,
        record: Rc<RefCell<Record>>,
        resource: u64,
        name: Box<str>,
    ) -> Reply {
        let (id, scope, key, parent) = {
            let record = record.borrow();
            (record.id, record.scope.borrow().id, record.subject, record.parent)
        };
        shared.entries.push(Entry {
            key: resource,
            parent: 2,
            name,
            kind: Kind::Work { scope, request: id, retained: false },
            activation: None,
            receiver: None,
            group: None,
            ownership: None,
        });
        shared.requests.push(Rc::clone(&record));
        shared.emit(id, scope, key, Stage::Admitted, parent);
        Box::pin(Wait {
            id,
            shared: Rc::clone(&self.shared),
            scope: Rc::clone(&self.scope),
            record: Some(record),
            recovery: false,
            generation: self.generation,
        })
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
        shared: &mut Shared,
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
                let readers = record.borrow().readers;
                let readers = readers
                    .checked_add(1)
                    .filter(|count| *count <= shared.limits.requests)
                    .ok_or(Error::PRESSURE)?;
                let id = shared.id()?;
                record.borrow_mut().readers = readers;
                return Ok(Box::pin(Wait {
                    id,
                    shared: Rc::clone(&self.shared),
                    scope: Rc::clone(&self.scope),
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
        key: u64,
        request: u64,
    ) -> Result<Option<Rc<RefCell<Record>>>, Error> {
        if !self
            .scope
            .borrow()
            .permissions(shared, key)
            .is_some_and(|rights| rights.contains(Permissions::WRITE))
        {
            return Err(Error::DENIED);
        }
        if shared
            .requests
            .iter()
            .any(|record| record.borrow().references(ResourceId(key)))
        {
            return Err(Error::BUSY);
        }
        let record = shared
            .requests
            .iter()
            .find(|record| record.borrow().id == request)
            .ok_or(Error::UNAVAILABLE)?;
        if record.borrow().outcome.is_none() || record.borrow().readers != 0 {
            return Err(Error::BUSY);
        }
        record.borrow_mut().observed = true;
        Ok(shared.retire(request))
    }
}

fn validate_contract(
    target: &Kind,
    method: Method,
    input: &Message,
) -> Result<Box<[Permissions]>, Error> {
    match target {
        Kind::Session(_)
        | Kind::Snapshot(_)
        | Kind::ClosedSession { .. }
        | Kind::Participation(_) => {
            crate::sessions::surface(target, method, input)
        }
        Kind::Catalog(_) => crate::catalog::surface(method, &input.contract),
        Kind::Module(_) => crate::modules::surface(method, &input.contract),
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
                && crate::opening::contract(&input.contract) =>
        {
            crate::opening::validate_model(&model.borrow())?;
            Ok(Box::new([
                Permissions::ALL,
                Permissions::ALL,
                Permissions::empty(),
            ]))
        }
        Kind::Model(model)
            if method == Method::Write
                && (input.contract == model.borrow().create
                    || reovim_kernel_interface::session::is_create(
                        &input.contract,
                    )) =>
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
        Kind::ClosedBuffer { .. }
            if method == Method::Read
                && crate::describe::contract(&input.contract) =>
        {
            Err(Error::CLOSED)
        }
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
    if method == Method::Read
        && (crate::snapshots::contract(&input.contract)
            || crate::describe::contract(&input.contract))
    {
        if crate::snapshots::observing(&input.contract) {
            return Ok(Box::new([Permissions::READ; 4]));
        }
        if crate::describe::contract(&input.contract)
            && buffer.borrow().backing.is_some()
        {
            return Ok(Box::new([
                Permissions::READ,
                Permissions::READ,
                Permissions::empty(),
            ]));
        }
        return Ok(Box::new([Permissions::READ, Permissions::READ]));
    }
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
    if matches!(target, Kind::Participation(_)) {
        let slots = if input.contract == Contract::new("kernel.session.seek", 1)
        {
            &[Permissions::READ][..]
        } else {
            &[]
        };
        return translate_slots(shared, scope, slots, input);
    }
    if let Kind::Model(model) = target {
        let model = model.borrow();
        let slots = model.references.map_or_else(
            || model.initial_input.to_vec(),
            |rights| alloc::vec![rights; input.resources.len()],
        );
        return translate_slots(shared, scope, &slots, input);
    }
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
    if !provider.forwarding && surface.input.len() != input.resources.len() {
        return Err(Error::INVALID);
    }
    for (index, reference) in input.resources.iter_mut().enumerate() {
        let key = scope.key(*reference)?;
        let allowed = scope.permissions(shared, key).ok_or(Error::DENIED)?;
        if !provider.forwarding && !allowed.contains(surface.input[index]) {
            return Err(Error::DENIED);
        }
        *reference = ResourceId(key);
    }
    Ok(())
}

fn translate_slots(
    shared: &Shared,
    scope: &Scope,
    slots: &[Permissions],
    input: &mut Message,
) -> Result<(), Error> {
    if input.resources.len() != slots.len() {
        return Err(Error::INVALID);
    }
    for (reference, rights) in input.resources.iter_mut().zip(slots) {
        let key = scope.key(*reference)?;
        if !scope
            .permissions(shared, key)
            .is_some_and(|allowed| allowed.contains(*rights))
        {
            return Err(Error::DENIED);
        }
        *reference = ResourceId(key);
    }
    Ok(())
}
