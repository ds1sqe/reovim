use super::super::{Entry, Registry, access::Scope, codec, execution::Task};
use alloc::{
    boxed::Box,
    rc::{Rc, Weak},
    vec::Vec,
};
use core::cell::RefCell;
use reovim_kernel_interface::{
    Error, Outcome, Pending, Value, call::Completion, caller::Call,
    declaration::ValueLimits, provider::ResourceProvider,
};

pub(in crate::engine) struct Listing {
    registry: Weak<RefCell<Registry>>,
    owner: Weak<Entry>,
    cursor: u64,
    maximum: usize,
    non_ascii: bool,
}
pub(in crate::engine) fn admit(
    scope: &Scope,
    call: &Call,
) -> Result<Option<Task>, Error> {
    let handle = scope.live(call.subject)?;
    if handle.entry.directory {
        return Ok(None);
    }
    let namespace = handle
        .implementation
        .provider
        .try_borrow()
        .map_err(|_| Error::BUSY)?
        .namespace()
        .ok_or(Error::UNSUPPORTED)?;
    if !namespace.list {
        return Err(Error::UNSUPPORTED);
    }
    if handle.opened.borrow().held & 1 == 0 {
        return Err(Error::DENIED);
    }
    let input = codec::no_refs(&call.input)?;
    let (cursor, maximum) = input.split_at_checked(8).ok_or(Error::INVALID)?;
    let maximum = usize::try_from(codec::u64_value(maximum)?)
        .map_err(|_| Error::PRESSURE)?
        .min(scope.limits.values.bytes);
    if maximum < 12 {
        return Err(Error::PRESSURE);
    }
    Ok(Some(Task::listing(
        handle.entry.clone(),
        Listing {
            registry: scope.registry.clone(),
            owner: Rc::downgrade(&handle.entry),
            cursor: codec::u64_value(cursor)?,
            maximum,
            non_ascii: scope.non_ascii_names,
        },
    )))
}
impl Listing {
    pub fn begin(
        &self,
        provider: &mut ResourceProvider,
    ) -> Result<Pending, Error> {
        provider.list(self.cursor, self.maximum as u64)
    }
    pub fn validate(
        &self,
        completion: &mut Completion,
        limits: ValueLimits,
    ) -> Result<Option<Error>, Error> {
        if completion.activity & !1 != 0 {
            return Err(Error::PROVIDER);
        }
        match &mut completion.outcome {
            Outcome::Ok(value) => self.install(value, limits),
            Outcome::Unknown { progress: Some(_), .. } => Err(Error::PROVIDER),
            _ => Ok(None),
        }
    }
    fn install(
        &self,
        value: &mut Value,
        limits: ValueLimits,
    ) -> Result<Option<Error>, Error> {
        let page = decode(value, self.maximum, limits, self.non_ascii)?;
        if page.size > self.maximum {
            return Ok(Some(Error::PRESSURE));
        }
        let owner = self.owner.upgrade().ok_or(Error::CLOSED)?;
        let registry = self.registry.upgrade().ok_or(Error::CLOSED)?;
        let mut output = Vec::new();
        if output.try_reserve_exact(page.size).is_err() {
            return Ok(Some(Error::PRESSURE));
        }
        let ids = match super::intern::page(
            &mut registry.borrow_mut(),
            &owner,
            &page.keys,
        ) {
            Ok(ids) => ids,
            Err(error) => return Ok(Some(error)),
        };
        output.extend_from_slice(
            &u32::try_from(page.names.len()).unwrap().to_le_bytes(),
        );
        for (name, id) in page.names.iter().zip(ids) {
            output.extend_from_slice(
                &u32::try_from(name.len()).unwrap().to_le_bytes(),
            );
            output.extend_from_slice(name);
            output.extend_from_slice(&id.epoch);
            output.extend_from_slice(&id.serial.to_le_bytes());
        }
        output.extend_from_slice(&page.next.to_le_bytes());
        value.data = output.into_boxed_slice();
        Ok(None)
    }
}
struct Page {
    names: Vec<Box<[u8]>>,
    keys: Vec<Box<[u8]>>,
    next: u64,
    size: usize,
}
fn decode(
    value: &Value,
    maximum: usize,
    limits: ValueLimits,
    non_ascii: bool,
) -> Result<Page, Error> {
    let mut input = codec::no_refs(value)?;
    if input.len() > maximum {
        return Err(Error::PROVIDER);
    }
    let count = codec::u32_value(take(&mut input, 4)?)? as usize;
    if count > limits.elements.saturating_sub(3) / 3 {
        return Err(Error::PRESSURE);
    }
    let mut names: Vec<Box<[u8]>> = super::super::reserve(count)?;
    let mut keys = super::super::reserve(count)?;
    let mut size = 12_usize;
    for _ in 0..count {
        let name = bytes(&mut input)?;
        codec::name(name, non_ascii).map_err(|_| Error::PROVIDER)?;
        if names.iter().any(|old| old.as_ref() == name) {
            return Err(Error::PROVIDER);
        }
        size = size.checked_add(28 + name.len()).ok_or(Error::PRESSURE)?;
        names.push(name.into());
        keys.push(bytes(&mut input)?.into());
    }
    let next = codec::u64_value(input)?;
    Ok(Page { names, keys, next, size })
}
fn bytes<'a>(input: &mut &'a [u8]) -> Result<&'a [u8], Error> {
    let size = codec::u32_value(take(input, 4)?)? as usize;
    take(input, size)
}
fn take<'a>(input: &mut &'a [u8], count: usize) -> Result<&'a [u8], Error> {
    let (value, rest) = input.split_at_checked(count).ok_or(Error::PROVIDER)?;
    *input = rest;
    Ok(value)
}
