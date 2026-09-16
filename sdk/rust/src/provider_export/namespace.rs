use super::{calls, creation, owner};
use crate::raw;
use alloc::boxed::Box;
use core::ffi::c_void;
use reovim_kernel_interface::{
    Error, Pending, abi,
    provider::{NamespaceInfo, ResourceProvider},
};

pub(super) fn table(info: NamespaceInfo) -> abi::NamespaceOps {
    abi::NamespaceOps {
        bytes: size_of::<abi::NamespaceOps>(),
        flags: Some(flags),
        lookup: if info.lookup { Some(lookup) } else { None },
        list: if info.list { Some(list) } else { None },
        materialize: if info.materialize { Some(materialize) } else { None },
        create: if info.create { Some(create) } else { None },
        remove: if info.remove { Some(remove) } else { None },
        link: if info.link { Some(link) } else { None },
        rename: if info.rename { Some(rename) } else { None },
    }
}
unsafe extern "C" fn rename(
    context: *mut c_void,
    input: *const abi::Args,
    output: *mut abi::Pending,
    error: *mut abi::Error,
) -> u32 {
    if input.is_null() || output.is_null() || error.is_null() {
        return Error::INVALID.code.0;
    }
    // SAFETY: Native execution retains and serializes this provider owner.
    let Ok(owner) = (unsafe { owner(context) }) else {
        return Error::INVALID.code.0;
    };
    // SAFETY: Input and its spans remain readable through begin. raw::value
    // checks extents and copies them before the callback can retain data.
    let value = unsafe { raw::value(*input, owner.state.limits) };
    let result = value.and_then(|value| {
        let request = crate::namespace_value::decode(&value)?;
        owner
            .state
            .provider
            .try_borrow_mut()
            .map_err(|_| Error::BUSY)?
            .rename(request)
    });
    // SAFETY: Live writable output records were checked before any effect.
    unsafe { calls::complete(&owner.state, result, output, error) }
}
type Keyed = fn(&mut ResourceProvider, &[u8], &[u8]) -> Result<Pending, Error>;
unsafe extern "C" fn link(
    context: *mut c_void,
    name: abi::Bytes,
    key: abi::Bytes,
    output: *mut abi::Pending,
    error: *mut abi::Error,
) -> u32 {
    // SAFETY: The native caller retains context/spans and supplies writable
    // outputs; keyed validates their shape before use.
    unsafe { keyed(context, name, key, output, error, ResourceProvider::link) }
}
unsafe extern "C" fn remove(
    context: *mut c_void,
    name: abi::Bytes,
    key: abi::Bytes,
    output: *mut abi::Pending,
    error: *mut abi::Error,
) -> u32 {
    // SAFETY: The native caller retains context/spans and supplies writable
    // outputs; keyed validates their shape before use.
    unsafe {
        keyed(context, name, key, output, error, ResourceProvider::remove)
    }
}
unsafe fn keyed(
    context: *mut c_void,
    name: abi::Bytes,
    key: abi::Bytes,
    output: *mut abi::Pending,
    error: *mut abi::Error,
    callback: Keyed,
) -> u32 {
    if output.is_null() || error.is_null() {
        return Error::INVALID.code.0;
    }
    // SAFETY: Native execution retains and serializes this provider owner.
    let Ok(owner) = (unsafe { owner(context) }) else {
        return Error::INVALID.code.0;
    };
    // SAFETY: Native input spans remain readable through begin. elements
    // checks nullness and bounded lengths before either span is borrowed.
    let name = unsafe { raw::elements(name.ptr, name.len, 4096) };
    // SAFETY: The key shares that lifetime and obeys the admitted byte bound.
    let key =
        unsafe { raw::elements(key.ptr, key.len, owner.state.limits.bytes) };
    let result = name.and_then(|name| {
        key.and_then(|key| {
            let mut provider = owner
                .state
                .provider
                .try_borrow_mut()
                .map_err(|_| Error::BUSY)?;
            callback(&mut provider, name, key)
        })
    });
    // SAFETY: Live writable output records were checked before any effect.
    unsafe { calls::complete(&owner.state, result, output, error) }
}
unsafe extern "C" fn flags(
    context: *mut c_void,
    output: *mut abi::Pending,
    error: *mut abi::Error,
) -> u32 {
    if output.is_null() || error.is_null() {
        return Error::INVALID.code.0;
    }
    // SAFETY: The native caller retains and serializes this provider owner.
    let Ok(owner) = (unsafe { owner(context) }) else {
        return Error::INVALID.code.0;
    };
    let result = owner
        .state
        .provider
        .try_borrow_mut()
        .map_err(|_| Error::BUSY)
        .and_then(|mut provider| provider.namespace_flags());
    // SAFETY: Output/error records are live writable native records. Nullness
    // was checked before callback entry; complete transfers only accepted work.
    unsafe { calls::complete(&owner.state, result, output, error) }
}
unsafe extern "C" fn lookup(
    context: *mut c_void,
    name: abi::Bytes,
    output: *mut abi::Pending,
    error: *mut abi::Error,
) -> u32 {
    if output.is_null() || error.is_null() {
        return Error::INVALID.code.0;
    }
    // SAFETY: Native execution retains and serializes this provider owner.
    let Ok(owner) = (unsafe { owner(context) }) else {
        return Error::INVALID.code.0;
    };
    // SAFETY: The native caller supplies this immutable span through begin;
    // elements checks nullness and the namespace's maximum path extent.
    let result =
        unsafe { raw::elements(name.ptr, name.len, 4096) }.and_then(|name| {
            owner
                .state
                .provider
                .try_borrow_mut()
                .map_err(|_| Error::BUSY)?
                .lookup(name)
        });
    // SAFETY: Output records were checked before any callback could execute.
    unsafe { calls::complete(&owner.state, result, output, error) }
}
unsafe extern "C" fn list(
    context: *mut c_void,
    cursor: u64,
    maximum: u64,
    output: *mut abi::Pending,
    error: *mut abi::Error,
) -> u32 {
    if output.is_null() || error.is_null() {
        return Error::INVALID.code.0;
    }
    // SAFETY: Native execution retains and serializes this provider owner.
    let Ok(owner) = (unsafe { owner(context) }) else {
        return Error::INVALID.code.0;
    };
    let result = listed(&owner.state, cursor, maximum);
    // SAFETY: Native output/error point to live writable records, checked
    // before invocation; complete transfers pending ownership only on success.
    unsafe { calls::complete(&owner.state, result, output, error) }
}
fn listed(
    state: &super::State,
    cursor: u64,
    maximum: u64,
) -> Result<reovim_kernel_interface::Pending, Error> {
    if maximum > state.limits.bytes as u64 {
        return Err(Error::PRESSURE);
    }
    state
        .provider
        .try_borrow_mut()
        .map_err(|_| Error::BUSY)?
        .list(cursor, maximum)
}
unsafe extern "C" fn materialize(
    context: *mut c_void,
    key: abi::Bytes,
    staging: *const abi::Creation,
    output: *mut abi::Pending,
    error: *mut abi::Error,
) -> u32 {
    // SAFETY: Native caller retains context, spans and writable output records.
    unsafe {
        staged(
            context,
            key,
            staging,
            output,
            error,
            ResourceProvider::materialize,
        )
    }
}
unsafe extern "C" fn create(
    context: *mut c_void,
    name: abi::Bytes,
    staging: *const abi::Creation,
    output: *mut abi::Pending,
    error: *mut abi::Error,
) -> u32 {
    // SAFETY: Native caller retains context, spans and writable output records.
    unsafe {
        staged(
            context,
            name,
            staging,
            output,
            error,
            ResourceProvider::create_child,
        )
    }
}
type Staged = fn(
    &mut ResourceProvider,
    &[u8],
    &reovim_kernel_interface::provider::Creation,
) -> Result<Pending, Error>;
unsafe fn staged(
    context: *mut c_void,
    key: abi::Bytes,
    staging: *const abi::Creation,
    output: *mut abi::Pending,
    error: *mut abi::Error,
    callback: Staged,
) -> u32 {
    if staging.is_null() || output.is_null() || error.is_null() {
        return Error::INVALID.code.0;
    }
    // SAFETY: The native caller retains this serialized provider owner.
    let Ok(owner) = (unsafe { owner(context) }) else {
        return Error::INVALID.code.0;
    };
    let state = &owner.state;
    // SAFETY: Native begin supplies readable Creation and retains its context
    // through pending finish. The lease fences every safe retained clone.
    let (creation, lease) = unsafe { creation::import(*staging, state.limits) };
    // SAFETY: The key's initialized byte span lives through this callback;
    // elements validates nullness/extent before the provider can capture it.
    let result = unsafe { raw::elements(key.ptr, key.len, state.limits.bytes) }
        .and_then(|key| {
            callback(
                &mut *state
                    .provider
                    .try_borrow_mut()
                    .map_err(|_| Error::BUSY)?,
                key,
                &creation,
            )
        });
    // SAFETY: Output records were checked before callbacks. Accepted pending
    // retains the staging lease until finish; rejection expires its safe
    // clones.
    unsafe {
        calls::complete_owned(
            state,
            result,
            output,
            error,
            Some(Box::new(lease)),
        )
    }
}
