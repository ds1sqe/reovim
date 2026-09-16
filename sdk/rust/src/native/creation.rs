use alloc::rc::Rc;
use core::ffi::c_void;
use reovim_kernel_interface::{
    Error, abi, declaration::ValueLimits, provider::Creation,
};

use super::Instance;

struct State {
    creation: Creation,
    instance: Rc<Instance>,
    limits: ValueLimits,
}

pub(super) struct Export {
    _state: Rc<State>,
    pub raw: abi::Creation,
}

impl Export {
    pub fn new(
        creation: Creation,
        instance: Rc<Instance>,
        limits: ValueLimits,
    ) -> Rc<Self> {
        let state = Rc::new(State { creation, instance, limits });
        let raw = abi::Creation {
            keyed: Some(keyed),
            context: Rc::as_ptr(&state).cast_mut().cast(),
            resource: Some(resource),
            buffer: Some(buffer),
            retained: Some(retained),
            auxiliary: Some(auxiliary),
        };
        Rc::new(Self { _state: state, raw })
    }
}

unsafe extern "C" fn resource(
    context: *mut c_void,
    provider: *const abi::Provider,
) -> u32 {
    // SAFETY: Caller retains the creation context and readable descriptor.
    unsafe { stage(context, provider, None) }
}
unsafe extern "C" fn keyed(
    context: *mut c_void,
    key: abi::Bytes,
    provider: *const abi::Provider,
) -> u32 {
    // SAFETY: Caller retains context, key and provider through this callback.
    unsafe { stage(context, provider, Some(key)) }
}
unsafe fn stage(
    context: *mut c_void,
    provider: *const abi::Provider,
    key: Option<abi::Bytes>,
) -> u32 {
    if context.is_null() {
        return Error::INVALID.code.0;
    }
    // SAFETY: Export supplies this Rc-stable State address and pending/result
    // owners retain Export through every serialized native callback. The native
    // factory supplies readable provider records under its retained instance.
    let state = unsafe { &*context.cast::<State>() };
    // SAFETY: Staging borrows the descriptor without taking its native context.
    // Its callbacks/storage share the factory instance's retained code lease.
    let prepared = unsafe {
        super::prepare(provider, state.instance.clone(), state.limits)
    };
    let (provider, instance) = match prepared {
        Ok(prepared) => prepared,
        Err(error) => return error.code.0,
    };
    // SAFETY: The optional key is readable through staging; elements checks
    // nullness and extent before borrowing, and the staging owner copies it.
    let key = match key
        .map(|key| unsafe {
            crate::raw::elements(key.ptr, key.len, state.limits.bytes)
        })
        .transpose()
    {
        Ok(key) => key,
        Err(error) => return error.code.0,
    };
    let result = match key {
        Some(key) => state.creation.keyed_resource(key, provider),
        None => state.creation.resource(provider),
    };
    match result {
        Ok(()) => {
            // Only successful staging transfers the native context. A rejected
            // temporary adapter must neither stop nor destroy the supplier's
            // provider, whose descriptor and ownership remain unchanged.
            instance.owned.set(true);
            0
        }
        Err((error, _provider)) => error.code.0,
    }
}

const unsafe extern "C" fn buffer(
    _: *mut c_void,
    _: *const abi::BufferInit,
) -> u32 {
    Error::UNSUPPORTED.code.0
}

const unsafe extern "C" fn retained(
    _: *mut c_void,
    _: *const abi::StateInit,
) -> u32 {
    Error::UNSUPPORTED.code.0
}

unsafe extern "C" fn auxiliary(
    context: *mut c_void,
    provider: *const abi::Provider,
    output: *mut u64,
) -> u32 {
    if context.is_null() || output.is_null() {
        return Error::INVALID.code.0;
    }
    // SAFETY: Export retains this stable State through the serialized native
    // request. The supplier keeps the provider descriptor readable and output
    // points to writable aligned token storage for this staging call.
    let state = unsafe { &*context.cast::<State>() };
    // SAFETY: Validate under the factory's retained code lease without taking
    // the supplied native context until its typed staging callback accepts it.
    let prepared = unsafe {
        super::prepare(provider, state.instance.clone(), state.limits)
    };
    let (provider, instance) = match prepared {
        Ok(prepared) => prepared,
        Err(error) => return error.code.0,
    };
    match state.creation.auxiliary(provider) {
        Ok(token) => {
            instance.owned.set(true);
            // SAFETY: Staging accepted ownership and output is caller-provided
            // writable u64 storage. Rejections never modify this destination.
            unsafe { output.write(token.0) };
            0
        }
        Err((error, _provider)) => error.code.0,
    }
}
