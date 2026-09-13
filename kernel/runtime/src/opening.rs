//! Install source/base ownership with initial working state, never afterwards.
use crate::{Shared, namespace::Kind, request::Record};
use alloc::rc::Rc;
use core::cell::RefCell;
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, ResourceId, buffer::ModelProvider,
    provider::Permissions,
};

pub struct Association {
    pub source: u64,
    pub published: usize,
    _source: Connection,
    _base: Connection,
}

pub fn contract(value: &Contract) -> bool {
    value == &Contract::new("kernel.buffer.open", 1)
}

pub fn prepare(
    shared: &Rc<RefCell<Shared>>,
    record: &Rc<RefCell<Record>>,
    model: &ModelProvider,
    mut input: Message,
) -> Result<Message, Error> {
    if !contract(&input.contract) {
        return Ok(input);
    }
    validate_model(model)?;
    let [source] = input.resources.as_ref() else {
        return Err(Error::INVALID);
    };
    let source = source.0;
    let scope = record.borrow().scope.clone();
    let local = scope.borrow_mut().export(&mut shared.borrow_mut(), source)?;
    let generation = scope.borrow().generation;
    let caller =
        crate::access::connection(shared.clone(), scope.clone(), generation);
    let source_access = caller.capture(local, Permissions::READ)?;
    let (base, base_access) =
        crate::read_views::acquire_root(shared, &scope, local)?;
    require_base(&shared.borrow(), base)?;
    input.contract = model.create.clone();
    input.resources = alloc::boxed::Box::new([ResourceId(base)]);
    record.borrow_mut().backing = Some(Association {
        source,
        published: 0,
        _source: source_access,
        _base: base_access,
    });
    Ok(input)
}

pub fn validate_model(model: &ModelProvider) -> Result<(), Error> {
    if model.references.is_some()
        || model.initial_input.as_ref() != [Permissions::READ]
    {
        return Err(Error::UNSUPPORTED);
    }
    Ok(())
}

pub fn source(record: &Record) -> Result<Option<u64>, Error> {
    if !contract(&record.input.contract) {
        return Ok(None);
    }
    Ok(Some(record.backing.as_ref().ok_or(Error::PROVIDER)?.source))
}

pub fn reply(limit: usize) -> Result<Message, Error> {
    if limit < 8 {
        return Err(Error::PRESSURE);
    }
    let mut reply =
        Message::new(Contract::new("kernel.buffer.open", 1), &[0; 8]);
    reply.resources = alloc::boxed::Box::new([ResourceId(0); 3]);
    Ok(reply)
}

pub fn stamp(reply: &mut Message, buffer: &crate::Editing, source: u64) {
    reply.data.copy_from_slice(&buffer.state.revision().to_le_bytes());
    reply.resources[0] = ResourceId(buffer.resource);
    reply.resources[1] = ResourceId(buffer.history);
    reply.resources[2] = ResourceId(source);
}

fn require_base(shared: &Shared, key: u64) -> Result<(), Error> {
    let Kind::ReadView { provider, .. } =
        &shared.entries[shared.entry_index(key)?].kind
    else {
        return Err(Error::PROVIDER);
    };
    if !provider
        .borrow()
        .surfaces
        .iter()
        .any(|surface| surface.contract == Contract::new("resource.base", 1))
    {
        return Err(Error::UNSUPPORTED);
    }
    Ok(())
}

pub fn revalidate(
    shared: &Shared,
    association: Option<&Association>,
) -> Result<(), Error> {
    let Some(association) = association else { return Ok(()) };
    let source = &shared.entries[shared.entry_index(association.source)?];
    if source.closed() {
        return Err(Error::CLOSED);
    }
    crate::modules::require_ready(source.activation.as_deref())
}
