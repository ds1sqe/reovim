use super::{Imported, complete_begin, creation, raw};
use alloc::rc::Rc;
use core::ptr;
use reovim_kernel_interface::{
    Error, Pending, abi,
    provider::{Creation, NamespaceOps},
};

pub(super) fn operations(
    table: abi::NamespaceOps,
) -> Result<NamespaceOps<Imported>, Error> {
    if table.flags.is_none() {
        return Err(Error::INVALID);
    }
    Ok(NamespaceOps {
        flags,
        create: table.create.map(|_| create as _),
        lookup: table.lookup.map(|_| lookup as _),
        list: table.list.map(|_| list as _),
        materialize: table.materialize.map(|_| materialize as _),
        remove: table.remove.map(|_| remove as _),
        link: table.link.map(|_| link as _),
        rename: table.rename.map(|_| rename as _),
    })
}
fn rename(
    state: &mut Imported,
    request: reovim_kernel_interface::provider::Rename<'_>,
) -> Result<Pending, Error> {
    let callback = table(state)?.rename.ok_or(Error::UNSUPPORTED)?;
    let value = crate::namespace_value::encode(request, state.limits.bytes)?;
    let args = abi::Args { data: raw::bytes(&value.data), ..raw::empty() };
    let mut pending = empty();
    let mut error = abi::Error::default();
    // SAFETY: The checked provider callback/context and immutable args remain
    // live through this serialized call. Pending native work captures inputs;
    // initialized output records remain writable through callback return.
    let result = unsafe {
        callback(
            state.instance.provider.context,
            &raw const args,
            &raw mut pending,
            &raw mut error,
        )
    };
    complete_begin(state, pending, result, error, None)
}
type Keyed = unsafe extern "C" fn(
    *mut core::ffi::c_void,
    abi::Bytes,
    abi::Bytes,
    *mut abi::Pending,
    *mut abi::Error,
) -> u32;
fn link(
    state: &mut Imported,
    name: &[u8],
    key: &[u8],
) -> Result<Pending, Error> {
    let callback = table(state)?.link.ok_or(Error::UNSUPPORTED)?;
    keyed(state, name, key, callback)
}
fn remove(
    state: &mut Imported,
    name: &[u8],
    key: &[u8],
) -> Result<Pending, Error> {
    let callback = table(state)?.remove.ok_or(Error::UNSUPPORTED)?;
    keyed(state, name, key, callback)
}
fn keyed(
    state: &mut Imported,
    name: &[u8],
    key: &[u8],
    callback: Keyed,
) -> Result<Pending, Error> {
    if key.len() > state.limits.bytes {
        return Err(Error::PRESSURE);
    }
    let mut pending = empty();
    let mut error = abi::Error::default();
    // SAFETY: Import retains validated callback/context/code. Name and key
    // remain readable through this call; native pending work captures them.
    // Initialized output records remain writable through the serialized call.
    let result = unsafe {
        callback(
            state.instance.provider.context,
            raw::bytes(name),
            raw::bytes(key),
            &raw mut pending,
            &raw mut error,
        )
    };
    complete_begin(state, pending, result, error, None)
}
fn flags(state: &mut Imported) -> Result<Pending, Error> {
    let callback = table(state)?.flags.ok_or(Error::UNSUPPORTED)?;
    let mut pending = empty();
    let mut error = abi::Error::default();
    // SAFETY: Import retains the validated callback, provider context and code.
    // Initialized output records remain writable through this serialized call.
    let result = unsafe {
        callback(
            state.instance.provider.context,
            &raw mut pending,
            &raw mut error,
        )
    };
    complete_begin(state, pending, result, error, None)
}
fn lookup(state: &mut Imported, name: &[u8]) -> Result<Pending, Error> {
    let callback = table(state)?.lookup.ok_or(Error::UNSUPPORTED)?;
    let mut pending = empty();
    let mut error = abi::Error::default();
    // SAFETY: Import copied the checked table and retains context/code. Name
    // bytes and initialized output records live through this serialized call;
    // pending native work must capture any input it retains.
    let result = unsafe {
        callback(
            state.instance.provider.context,
            raw::bytes(name),
            &raw mut pending,
            &raw mut error,
        )
    };
    complete_begin(state, pending, result, error, None)
}
fn list(
    state: &mut Imported,
    cursor: u64,
    maximum: u64,
) -> Result<Pending, Error> {
    let callback = table(state)?.list.ok_or(Error::UNSUPPORTED)?;
    if maximum > state.limits.bytes as u64 {
        return Err(Error::PRESSURE);
    }
    let mut pending = empty();
    let mut error = abi::Error::default();
    // SAFETY: The validated callback/context remain owned by instance. Scalar
    // inputs and writable output records are valid for the serialized call.
    let result = unsafe {
        callback(
            state.instance.provider.context,
            cursor,
            maximum,
            &raw mut pending,
            &raw mut error,
        )
    };
    complete_begin(state, pending, result, error, None)
}
fn materialize(
    state: &mut Imported,
    key: &[u8],
    staging: &Creation,
) -> Result<Pending, Error> {
    let callback = table(state)?.materialize.ok_or(Error::UNSUPPORTED)?;
    staged(state, key, staging, callback)
}
fn create(
    state: &mut Imported,
    name: &[u8],
    staging: &Creation,
) -> Result<Pending, Error> {
    let callback = table(state)?.create.ok_or(Error::UNSUPPORTED)?;
    staged(state, name, staging, callback)
}
type Staged = unsafe extern "C" fn(
    *mut core::ffi::c_void,
    abi::Bytes,
    *const abi::Creation,
    *mut abi::Pending,
    *mut abi::Error,
) -> u32;
fn staged(
    state: &mut Imported,
    key: &[u8],
    staging: &Creation,
    callback: Staged,
) -> Result<Pending, Error> {
    if key.len() > state.limits.bytes {
        return Err(Error::PRESSURE);
    }
    let owner = creation::Export::new(
        staging.clone(),
        state.instance.clone(),
        state.limits,
    );
    let mut pending = empty();
    let mut error = abi::Error::default();
    // SAFETY: Import retained the checked callback/context. Key bytes and the
    // initialized outputs live through this call; native work captures key.
    // The Rc-stable creation export remains retained through pending cleanup.
    let result = unsafe {
        callback(
            state.instance.provider.context,
            raw::bytes(key),
            &raw const owner.raw,
            &raw mut pending,
            &raw mut error,
        )
    };
    complete_begin(
        state,
        pending,
        result,
        error,
        Some(owner as Rc<dyn core::any::Any>),
    )
}
fn table(state: &Imported) -> Result<abi::NamespaceOps, Error> {
    if state.instance.stopped.get() {
        return Err(Error::CLOSED);
    }
    state.namespace.ok_or(Error::UNSUPPORTED)
}
const fn empty() -> abi::Pending {
    abi::Pending { context: ptr::null_mut(), ops: ptr::null() }
}
