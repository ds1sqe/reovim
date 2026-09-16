use super::super::{
    Entry,
    access::{Opened, Scope},
    codec,
    execution::Task,
};
use alloc::{boxed::Box, rc::Rc};
use core::cell::RefCell;
use reovim_kernel_interface::{
    Error, Outcome, Pending, call::Completion, caller::Call,
    provider::ResourceProvider,
};

pub(in crate::engine) struct Link {
    _source: Rc<Entry>,
    opened: Rc<RefCell<Opened>>,
    key: Box<[u8]>,
    name: Box<[u8]>,
}
pub(in crate::engine) fn admit(
    scope: &Scope,
    call: &Call,
) -> Result<Task, Error> {
    let parent = scope.live(call.subject)?;
    if parent.opened.borrow().held & 2 == 0 {
        return Err(Error::DENIED);
    }
    let namespace = parent
        .implementation
        .provider
        .try_borrow()
        .map_err(|_| Error::BUSY)?
        .namespace()
        .ok_or(Error::UNSUPPORTED)?;
    if !namespace.link {
        return Err(Error::UNSUPPORTED);
    }
    let (slot, data) =
        call.input.data.split_at_checked(4).ok_or(Error::INVALID)?;
    if codec::u32_value(slot)? != 0 || call.input.references.len() != 1 {
        return Err(Error::INVALID);
    }
    let name = codec::bytes(data, 4096)?;
    codec::name(name, scope.non_ascii_names)?;
    let source = scope.live(call.input.references[0])?;
    let registry = scope.registry.upgrade().ok_or(Error::CLOSED)?;
    let key = super::intern::selected(
        &registry.borrow(),
        &parent.entry,
        &source.entry,
    )?
    .ok_or(Error::UNSUPPORTED)?;
    Ok(Task::link(
        parent.entry.clone(),
        Link {
            _source: source.entry.clone(),
            opened: source.opened.clone(),
            key,
            name: name.into(),
        },
    ))
}
impl Link {
    pub fn begin(
        &self,
        provider: &mut ResourceProvider,
    ) -> Result<Pending, Error> {
        provider.link(&self.name, &self.key)
    }
    pub fn uses(&self, opened: &Rc<RefCell<Opened>>) -> bool {
        Rc::ptr_eq(&self.opened, opened)
    }
    pub fn validate(completion: &Completion) -> Result<Option<Error>, Error> {
        if completion.activity & !2 != 0 {
            return Err(Error::PROVIDER);
        }
        match &completion.outcome {
            Outcome::Ok(value)
                if !value.data.is_empty() || !value.references.is_empty() =>
            {
                Err(Error::PROVIDER)
            }
            Outcome::Unknown { progress: Some(_), .. } => Err(Error::PROVIDER),
            _ => Ok(None),
        }
    }
}
