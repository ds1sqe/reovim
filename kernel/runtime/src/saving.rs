//! A save captures state/base at admission and keeps its original Work owner.
use crate::{
    Editing, Shared,
    namespace::{Kind, Scope},
    opening::Base,
};
use alloc::{boxed::Box, rc::Rc};
use core::cell::RefCell;
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, RequestError, ResourceId,
    access::Reply,
    buffer::{ModelProvider, State},
    provider::Permissions,
    publication::backend::{self, Status},
};

mod dispatch;

pub struct Save {
    pub buffer: Rc<RefCell<Editing>>,
    state: State,
    model: Rc<RefCell<ModelProvider>>,
    pub source: u64,
    _target: Kind,
    base: Rc<Base>,
    revision: u64,
    node: usize,
    parameters: Box<[u8]>,
    context: Option<(Rc<RefCell<Scope>>, Connection)>,
    pub custodian: Option<Rc<RefCell<Scope>>>,
    receipt: Option<backend::Receipt>,
    evidence: Option<(u64, Kind)>,
    publisher: Option<u64>,
    release_work: Option<u64>,
    pub handed: bool,
    pub cancelled: bool,
    pub settled: bool,
    pub last_error: Option<Error>,
    release: Release,
    status: Option<Status>,
    work: u64,
    recovery: Box<str>,
    reply: Option<Message>,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Release {
    Held,
    Acknowledged,
    Done,
}

pub fn contract(input: &Contract) -> bool {
    input == &Contract::new("kernel.buffer.save", 1)
}

pub fn capture(
    shared: &Shared,
    scope: &Scope,
    target: &Kind,
    input: &Message,
) -> Result<Option<Rc<RefCell<Save>>>, Error> {
    if !contract(&input.contract) {
        return Ok(None);
    }
    let Kind::Buffer(buffer) = target else {
        return Err(Error::INVALID);
    };
    let value = buffer.borrow();
    let revision = crate::saving::revision(&input.data)?;
    if value.state.revision() != revision {
        return Err(Error::STALE);
    }
    let association = value.backing.as_ref().ok_or(Error::UNAVAILABLE)?;
    if !scope
        .permissions(shared, association.source)
        .is_some_and(|rights| rights.contains(Permissions::WRITE))
    {
        return Err(Error::DENIED);
    }
    let entry = &shared.entries[shared.entry_index(association.source)?];
    crate::modules::require_ready(entry.activation.as_deref())?;
    if entry.closed() {
        return Err(Error::CLOSED);
    }
    validate(&entry.kind)?;
    Ok(Some(Rc::new(RefCell::new(Save {
        buffer: buffer.clone(),
        state: value.state.state(),
        model: value.model.clone(),
        source: association.source,
        _target: entry.kind.clone(),
        base: association.base.clone(),
        revision,
        node: value.state.node(),
        parameters: input.data[8..].into(),
        context: None,
        custodian: None,
        receipt: None,
        evidence: None,
        publisher: None,
        release_work: None,
        handed: false,
        cancelled: false,
        settled: false,
        last_error: None,
        release: Release::Done,
        status: None,
        work: 0,
        recovery: "".into(),
        reply: None,
    }))))
}

fn revision(data: &[u8]) -> Result<u64, Error> {
    Ok(u64::from_le_bytes(
        data.get(..8)
            .ok_or(Error::INVALID)?
            .try_into()
            .map_err(|_| Error::INVALID)?,
    ))
}

fn validate(target: &Kind) -> Result<(), Error> {
    let Kind::Provider(provider) = target else {
        return Err(Error::UNSUPPORTED);
    };
    let provider = provider.borrow();
    if provider.forwarding
        || !provider.surfaces.iter().any(|surface| {
            surface.contract == Contract::new("resource.publish", 1)
                && surface.methods.contains(Permissions::WRITE)
                && surface.input.as_ref()
                    == [Permissions::READ, Permissions::READ]
                && surface.output.as_ref() == [Permissions::ALL]
        })
        || !provider.surfaces.iter().any(|surface| {
            surface.contract == Contract::new("resource.effect.release", 1)
                && surface.methods.contains(Permissions::WRITE)
                && surface.input.is_empty()
                && surface.output.is_empty()
        })
    {
        return Err(Error::UNSUPPORTED);
    }
    Ok(())
}

pub fn start(
    shared: Rc<RefCell<Shared>>,
    save: Rc<RefCell<Save>>,
    owner: u64,
    work: u64,
    recovery: Box<str>,
) -> Reply {
    let result = prepare(&shared, &save, owner, work, recovery);
    Box::pin(async move {
        result?;
        dispatch::run(&shared, &save, false).await
    })
}

fn prepare(
    shared: &Rc<RefCell<Shared>>,
    save: &RefCell<Save>,
    owner: u64,
    work: u64,
    recovery: Box<str>,
) -> Result<(), Error> {
    let mut save = save.borrow_mut();
    save.work = work;
    save.recovery = recovery;
    if save.recovery.len().saturating_add(19)
        > shared.borrow().limits.message_bytes
    {
        return Err(Error::PRESSURE);
    }
    reserve_reply(&mut save)?;
    let provider =
        crate::snapshots::selected(save.model.clone(), save.state.clone());
    let (_, scope, connection) =
        crate::read_views::publish(shared, owner, provider)?;
    scope.borrow_mut().grants.try_reserve(2).map_err(|_| Error::PRESSURE)?;
    scope.borrow_mut().grants.extend([
        (save.source, Permissions::WRITE),
        (save.base.key, Permissions::READ),
    ]);
    save.context = Some((scope, connection));
    Ok(())
}

pub fn resume(
    shared: Rc<RefCell<Shared>>,
    save: Rc<RefCell<Save>>,
    release: bool,
) -> Result<Reply, Error> {
    reserve_reply(&mut save.borrow_mut())?;
    save.borrow_mut().last_error = None;
    Ok(Box::pin(async move {
        if release {
            dispatch::release(&shared, &save).await
        } else {
            dispatch::run(&shared, &save, true).await
        }
    }))
}

pub fn adopt(
    shared: &Shared,
    record: &RefCell<crate::request::Record>,
    scope: &Rc<RefCell<Scope>>,
    rights: Permissions,
) -> Result<Message, Error> {
    if shared.closing {
        return Err(Error::CLOSED);
    }
    if !rights.contains(Permissions::ALL) {
        return Err(Error::DENIED);
    }
    let record = record.borrow();
    let save = record.save.as_ref().ok_or(Error::UNSUPPORTED)?;
    if record.work.is_some() || record.readers != 0 {
        return Err(Error::BUSY);
    }
    let mut save = save.borrow_mut();
    let (context, _) = save.context.as_ref().ok_or(Error::UNAVAILABLE)?;
    let reply =
        Message::new(Contract::new("kernel.request.adopt", 1), b"adopted");
    context.borrow_mut().owner = scope.borrow().owner;
    save.custodian = Some(scope.clone());
    Ok(reply)
}

fn reserve_reply(save: &mut Save) -> Result<(), Error> {
    let mut data = alloc::vec::Vec::new();
    data.try_reserve_exact(19 + save.recovery.len())
        .map_err(|_| Error::PRESSURE)?;
    data.resize(19, 0);
    data[..8].copy_from_slice(&save.revision.to_le_bytes());
    data.extend_from_slice(save.recovery.as_bytes());
    save.reply = Some(Message {
        contract: Contract::new("kernel.buffer.save", 1),
        data: data.into(),
        resources: Box::new([
            ResourceId(save.buffer.borrow().resource),
            ResourceId(save.source),
            ResourceId(save.work),
        ]),
    });
    Ok(())
}

fn message(save: &mut Save) -> Result<Message, Error> {
    let status = save.status.ok_or(Error::NOT_READY)?;
    let mut reply = save.reply.take().ok_or(Error::PROVIDER)?;
    let (length, tag, cause, violation) = match status {
        Status::Applied => (1, 0, 0, false),
        Status::DefinitelyNotApplied(cause) => (9, 1, cause.0, false),
        Status::OutcomeUnknown(cause) => (9, 2, cause.0, false),
        Status::PartiallyApplied { cause, atomicity_violated } => {
            (10, 3, cause.0, atomicity_violated)
        }
    };
    reply.data[8] = length;
    reply.data[9] = tag;
    reply.data[10..18].copy_from_slice(&cause.to_le_bytes());
    reply.data[18] = u8::from(violation);
    Ok(reply)
}

const fn cause(error: &RequestError) -> Error {
    match error {
        RequestError::Rejected(error) | RequestError::Failed(error) => *error,
        RequestError::Unknown { cause, .. } => *cause,
    }
}

impl Save {
    pub const fn status(&self) -> &'static [u8] {
        if self.settled {
            return b"finished";
        }
        if matches!(
            self.status,
            Some(Status::OutcomeUnknown(_) | Status::PartiallyApplied { .. })
        ) {
            return b"unknown";
        }
        b"pending"
    }
}

pub fn rejected(shared: &mut Shared, record: &crate::request::Record) {
    let Some(save) = &record.save else {
        return;
    };
    if save.borrow().handed {
        return;
    }
    save.borrow_mut().settled = true;
    for entry in &mut shared.entries {
        if let Kind::Work { request, retained, .. } = &mut entry.kind
            && *request == record.id
        {
            *retained = false;
        }
    }
}

pub fn retain(shared: &mut Shared, id: u64) -> Result<u64, Error> {
    let entry = shared.entries.iter_mut().find(|entry| {
        matches!(entry.kind, Kind::Work { request, .. } if request == id)
    }).ok_or(Error::UNAVAILABLE)?;
    if let Kind::Work { retained, .. } = &mut entry.kind {
        *retained = true;
    }
    Ok(entry.key)
}

pub fn prepare_close(
    shared: &Rc<RefCell<Shared>>,
    record: &Rc<RefCell<crate::request::Record>>,
) -> Result<(), Error> {
    let save = record.borrow().save.clone();
    let Some(save) = save else {
        return Ok(());
    };
    if save.borrow().handed && !save.borrow().settled {
        return Err(Error::BUSY);
    }
    if save.borrow().release == Release::Done {
        return Ok(());
    }
    if record.borrow().work.is_none() {
        let pending = resume(shared.clone(), save, true)?;
        record.borrow_mut().work =
            Some(crate::request::Work::Resource(pending));
        record.borrow_mut().schedule = crate::request::Schedule::new();
    }
    Err(Error::BUSY)
}
