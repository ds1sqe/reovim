use crate::{
    Shared,
    namespace::{Kind, Scope},
};
use alloc::{boxed::Box, rc::Rc, vec::Vec};
use core::cell::RefCell;
use reovim_kernel_interface::{
    Error, Message, ResourceId, buffer::ModelProvider, provider::Permissions,
};

pub struct Value {
    pub message: Message,
    pub rights: Box<[Permissions]>,
    _subjects: Vec<Kind>,
}

pub fn capture(
    shared: &Shared,
    receiver: Option<&Rc<RefCell<Scope>>>,
    model: &ModelProvider,
    mut message: Message,
) -> Result<Value, Error> {
    if message.resources.len() > shared.limits.resources {
        return Err(Error::PRESSURE);
    }
    if message.resources.is_empty() {
        return Ok(Value {
            message,
            rights: Box::new([]),
            _subjects: Vec::new(),
        });
    }
    let rights = model.references.ok_or(Error::PROVIDER)?;
    let scope = receiver.ok_or(Error::PROVIDER)?.borrow();
    for id in &mut message.resources {
        let key = scope.key(*id).map_err(|_| Error::PROVIDER)?;
        if !scope.permissions(shared, key).is_some_and(|p| p.contains(rights)) {
            return Err(Error::PROVIDER);
        }
        *id = ResourceId(key);
    }
    let subjects = crate::binding::capture(shared, &message)?;
    let rights = alloc::vec![rights; message.resources.len()].into();
    Ok(Value { message, rights, _subjects: subjects })
}

pub fn set_output(
    record: &Rc<RefCell<crate::request::Record>>,
    value: &Value,
) -> Result<(), Error> {
    if value.rights.is_empty() {
        return Ok(());
    }
    let mut record = record.borrow_mut();
    if record.output.len() != 2 {
        return Err(Error::PROVIDER);
    }
    let mut rights = Vec::new();
    rights.try_reserve(2 + value.rights.len()).map_err(|_| Error::PRESSURE)?;
    rights.extend_from_slice(&record.output);
    rights.extend_from_slice(&value.rights);
    record.output = rights.into();
    Ok(())
}
