use alloc::{
    boxed::Box,
    rc::{Rc, Weak},
    vec::Vec,
};
use core::cell::RefCell;
use reovim_kernel_interface::{
    Error, Outcome, ResourceId, Value,
    caller::{Call, Operation},
    declaration::{Declaration, Method, Operation as Declared, ValueType},
    resource::AccessId,
};

use super::{
    Entry, Grant, Limits, Registry, codec, custody::Record, execution::Task,
    factory::Factory, loans::Loans, outputs::Outputs, reserve,
};

#[derive(Clone, Copy)]
pub(super) enum Ownership {
    Owned { retain_loans: bool },
    Loan { capture: bool },
}

impl Ownership {
    pub const fn captures(self) -> bool {
        match self {
            Self::Owned { .. } => true,
            Self::Loan { capture } => capture,
        }
    }

    pub const fn retains_loans(self) -> bool {
        match self {
            Self::Owned { retain_loans } => retain_loans,
            Self::Loan { capture } => capture,
        }
    }
}

pub(super) struct Opened {
    pub held: u32,
    pub ceiling: u32,
    pub create_access: u32,
    pub closed: bool,
    pub ownership: Ownership,
}

pub(super) struct Handle {
    pub token: AccessId,
    pub implementation: Rc<super::Implementation>,
    pub entry: Rc<Entry>,
    pub opened: Rc<RefCell<Opened>>,
    pub interface: Option<usize>,
    pub provisional: Option<u64>,
}

pub(super) struct Scope {
    pub registry: Weak<RefCell<Registry>>,
    pub limits: Limits,
    pub ready: bool,
    pub closed: bool,
    pub grants: Vec<(Rc<Entry>, Grant)>,
    pub publications: Vec<(Rc<Entry>, super::PublicationGrant)>,
    pub handles: Vec<Handle>,
    pub records: Vec<Rc<RefCell<Record>>>,
    pub next_request: u64,
    pub next_handle: u64,
    pub reserved_handles: usize,
    pub aliases: Option<Vec<(Box<[u8]>, reovim_kernel_interface::ResourceId)>>,
    pub non_ascii_names: bool,
}

impl Scope {
    pub fn new(
        grants: &[Grant],
        registry: &Rc<RefCell<Registry>>,
        limits: Limits,
        ready: bool,
        non_ascii_names: bool,
    ) -> Result<Self, Error> {
        if grants.len() > limits.resources {
            return Err(Error::PRESSURE);
        }
        let mut captured = reserve(grants.len())?;
        for &grant in grants {
            capture_grant(&mut captured, &registry.borrow().entries, grant)?;
        }
        Ok(Self {
            registry: Rc::downgrade(registry),
            limits,
            ready,
            closed: false,
            grants: captured,
            publications: Vec::new(),
            handles: reserve(limits.handles)?,
            records: reserve(limits.requests)?,
            next_request: 1,
            next_handle: 1,
            reserved_handles: 0,
            aliases: None,
            non_ascii_names,
        })
    }

    pub fn live(&self, token: AccessId) -> Result<&Handle, Error> {
        let handle = self.opened(token)?;
        if handle.entry.is_closed() {
            return Err(Error::CLOSED);
        }
        Ok(handle)
    }

    pub fn opened(&self, token: AccessId) -> Result<&Handle, Error> {
        let handle = self
            .handles
            .iter()
            .find(|h| h.token == token)
            .ok_or(Error::DENIED)?;
        if handle.provisional.is_some() {
            return Err(Error::DENIED);
        }
        if handle.opened.borrow().closed {
            return Err(Error::CLOSED);
        }
        Ok(handle)
    }

    pub const fn token(&self) -> Result<AccessId, Error> {
        if self.handles.len() + self.reserved_handles == self.limits.handles
            || self.next_handle == u64::MAX
        {
            return Err(Error::PRESSURE);
        }
        Ok(AccessId(self.next_handle))
    }

    pub fn declared(
        &mut self,
        call: &Call,
        current: &Rc<RefCell<Self>>,
    ) -> Result<Task, Error> {
        let handle = self.live(call.subject)?;
        let interface = handle.interface.ok_or(Error::UNSUPPORTED)?;
        let operation = match call.operation {
            Operation::Exec => Declared::Exec,
            Operation::Ioctl => Declared::Ioctl(call.command),
            _ => return Err(Error::UNSUPPORTED),
        };
        let method = handle.implementation.interfaces[interface]
            .methods()
            .iter()
            .find(|method| method.operation() == operation)
            .ok_or(Error::UNSUPPORTED)?;
        let rights = self.input(handle, method, &call.input)?;
        let entry = handle.entry.clone();
        let implementation = handle.implementation.clone();
        let method = method.clone();
        let opened = handle.opened.clone();
        let mut output =
            Outputs::reserve(self, current, &entry, &method, call.request)?;
        let mut factory = Factory::reserve(
            self,
            current,
            &entry,
            &implementation,
            &opened.borrow(),
            &method,
        )
        .inspect_err(|_| output.release_in(self))?;
        let loans = Loans::admit(self, current, &entry, &call.input, &rights)
            .inspect_err(|_| {
            output.release_in(self);
            Factory::abort(&mut factory, self, current);
        })?;
        Ok(Task::new(entry, interface, method, loans, output, factory))
    }

    fn input(
        &self,
        handle: &Handle,
        method: &Method,
        input: &Value,
    ) -> Result<Vec<u32>, Error> {
        if handle.opened.borrow().held & method.access() != method.access() {
            return Err(Error::DENIED);
        }
        self.value_input(method.input(), input)
    }

    pub fn value_input(
        &self,
        schema: &ValueType,
        input: &Value,
    ) -> Result<Vec<u32>, Error> {
        let mut required = reserve(input.references.len())?;
        required.resize(input.references.len(), 0);
        schema.validate(
            &input.data,
            input.references.len(),
            self.limits.values,
            |slot, api, rights| {
                let slot = usize::try_from(slot).map_err(|_| Error::INVALID)?;
                self.reference(input.references[slot], api, rights)?;
                required[slot] |= rights;
                Ok(())
            },
        )?;
        Ok(required)
    }

    pub fn reference(
        &self,
        token: AccessId,
        api: Option<[u8; 16]>,
        rights: u32,
    ) -> Result<(), Error> {
        reference(self.live(token)?, api, rights)
    }

    pub fn common(&mut self, call: &Call) -> Result<Outcome, Error> {
        let data = codec::no_refs(&call.input)?;
        match call.operation {
            Operation::Acquire => acquire(self, call.request, data),
            Operation::Bind => bind(self, call, data),
            Operation::Interfaces => {
                super::discovery::interfaces(self, call.subject, data)
            }
            Operation::Entries => {
                super::namespace::entries(self, call.subject, data)
            }
            Operation::Info => super::info::info(self, call.subject, data),
            Operation::Capture => capture(self, call, data),
            Operation::SetAccess => set_access(self, call.subject, data),
            _ => Err(Error::UNSUPPORTED),
        }
    }
}

pub(super) fn capture_grant(
    captured: &mut Vec<(Rc<Entry>, Grant)>,
    entries: &[Rc<Entry>],
    grant: Grant,
) -> Result<(), Error> {
    if (grant.access | grant.create_access) & !7 != 0
        || captured.iter().any(|(entry, _)| entry.identity == grant.resource)
    {
        return Err(Error::INVALID);
    }
    let entry = entries
        .iter()
        .find(|entry| entry.identity == grant.resource)
        .ok_or(Error::DENIED)?;
    if entry.is_closed() {
        return Err(Error::CLOSED);
    }
    captured.push((entry.clone(), grant));
    Ok(())
}

fn acquire(
    scope: &mut Scope,
    request: u64,
    data: &[u8],
) -> Result<Outcome, Error> {
    let (tag, locator) = data.split_at_checked(4).ok_or(Error::INVALID)?;
    let (entry, grant) = locate(scope, codec::u32_value(tag)?, locator)?;
    if entry.is_closed() {
        return Err(Error::CLOSED);
    }
    let token = scope.token()?;
    let output = codec::acquired(entry.identity, token, scope.limits.values)?;
    let opened = Rc::new(RefCell::new(Opened {
        held: 0,
        ceiling: grant.access,
        create_access: grant.create_access,
        closed: false,
        ownership: Ownership::Owned { retain_loans: grant.retain_loans },
    }));
    scope.handles.push(Handle {
        token,
        implementation: entry.implementation.clone(),
        entry,
        opened,
        interface: None,
        provisional: Some(request),
    });
    scope.next_handle += 1;
    Ok(Outcome::Ok(output))
}

pub(super) fn locate(
    scope: &Scope,
    tag: u32,
    locator: &[u8],
) -> Result<(Rc<Entry>, Grant), Error> {
    let identity;
    let path;
    match tag {
        0 => {
            let bytes = codec::bytes(locator, 4096)?;
            codec::path(bytes, scope.non_ascii_names)?;
            identity = imported(scope, bytes)?;
            path = Some(bytes);
        }
        1 => {
            path = None;
            identity = Some(codec::identity(locator)?);
        }
        _ => return Err(Error::INVALID),
    }
    let matches = |entry: &Entry| {
        identity.map_or_else(
            || path == Some(entry.path.as_ref()),
            |id| entry.identity == id,
        )
    };
    if let Some((entry, grant)) =
        scope.grants.iter().find(|(entry, _)| matches(entry))
    {
        return Ok((entry.clone(), *grant));
    }
    // Private dependency aliases never fall back to publication authority.
    if path.is_some_and(|path| {
        scope.aliases.is_some() && path.starts_with(b"/imports/")
    }) {
        return Err(Error::DENIED);
    }
    let registry = scope.registry.upgrade().ok_or(Error::CLOSED)?;
    let registry = registry.borrow();
    let entry = registry
        .entries
        .iter()
        .find(|entry| matches(entry))
        .ok_or(Error::DENIED)?;
    let grant = scope
        .publications
        .iter()
        .find_map(|(owner, grant)| {
            (!owner.closed.get()).then(|| grant.member(entry)).flatten()
        })
        .ok_or(Error::DENIED)?;
    Ok((entry.clone(), grant))
}

fn imported(scope: &Scope, path: &[u8]) -> Result<Option<ResourceId>, Error> {
    let Some(aliases) = &scope.aliases else { return Ok(None) };
    if !path.starts_with(b"/imports/") {
        return Ok(None);
    }
    aliases
        .iter()
        .find(|(alias, _)| alias.as_ref() == path)
        .map(|(_, id)| Some(*id))
        .ok_or(Error::DENIED)
}

fn bind(scope: &mut Scope, call: &Call, data: &[u8]) -> Result<Outcome, Error> {
    let declaration = Declaration::decode(codec::bytes(data, 65536)?)?;
    let handle = scope.live(call.subject)?;
    let interface = handle
        .implementation
        .interfaces
        .iter()
        .position(|offered| offered == &declaration)
        .ok_or(Error::UNSUPPORTED)?;
    let token = scope.token()?;
    let output = Value::copy(&0_u32.to_le_bytes(), &[token])?;
    let bound = Handle {
        token,
        entry: handle.entry.clone(),
        implementation: handle.implementation.clone(),
        opened: handle.opened.clone(),
        interface: Some(interface),
        provisional: Some(call.request),
    };
    scope.handles.push(bound);
    scope.next_handle += 1;
    Ok(Outcome::Ok(output))
}

fn set_access(
    scope: &Scope,
    token: AccessId,
    data: &[u8],
) -> Result<Outcome, Error> {
    let rights = codec::u32_value(data)?;
    if rights & !7 != 0 {
        return Err(Error::INVALID);
    }
    let handle = scope.live(token)?;
    let mut opened = handle.opened.borrow_mut();
    if rights & opened.ceiling != rights {
        return Err(Error::DENIED);
    }
    opened.held = rights;
    Ok(Outcome::Ok(Value::unit()))
}

fn capture(
    scope: &mut Scope,
    call: &Call,
    data: &[u8],
) -> Result<Outcome, Error> {
    let rights = codec::u32_value(data)?;
    if rights & !7 != 0 {
        return Err(Error::INVALID);
    }
    let handle = scope.live(call.subject)?;
    let opened = handle.opened.borrow();
    if !opened.ownership.captures() || rights & opened.held != rights {
        return Err(Error::DENIED);
    }
    let retain_loans = opened.ownership.retains_loans();
    let create_access = opened.create_access;
    drop(opened);
    if scope.limits.values.bytes < 4 || scope.limits.values.references == 0 {
        return Err(Error::PRESSURE);
    }
    let token = scope.token()?;
    let output = Value::copy(&0_u32.to_le_bytes(), &[token])?;
    // Capture delegates the current mask, not the original grant ceiling.
    // Its fence and mask are independent; the subject/provider stays shared.
    let captured = Handle {
        token,
        entry: handle.entry.clone(),
        implementation: handle.implementation.clone(),
        opened: Rc::new(RefCell::new(Opened {
            held: rights,
            ceiling: rights,
            create_access,
            closed: false,
            ownership: Ownership::Owned { retain_loans },
        })),
        interface: handle.interface,
        provisional: Some(call.request),
    };
    scope.handles.push(captured);
    scope.next_handle += 1;
    Ok(Outcome::Ok(output))
}

pub(super) fn reference(
    handle: &Handle,
    api: Option<[u8; 16]>,
    rights: u32,
) -> Result<(), Error> {
    if handle.opened.borrow().closed {
        return Err(Error::CLOSED);
    }
    if handle.opened.borrow().held & rights != rights {
        return Err(Error::DENIED);
    }
    if api.is_some_and(|api| {
        !handle
            .implementation
            .interfaces
            .iter()
            .any(|offered| offered.api() == &api)
    }) {
        return Err(Error::UNSUPPORTED);
    }
    Ok(())
}
