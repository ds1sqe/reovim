//! Safe declarations for the existing synchronous immutable Model profile.

use super::Identity;
use alloc::boxed::Box;
use core::{ffi::c_void, ptr};
use reovim_kernel_interface::{Error, Message, abi, buffer::State};

pub struct ModelDefinition {
    pub identity: Identity,
    pub dependencies: &'static [Identity],
    pub initial: Identity,
    pub edit: Identity,
    pub read: Identity,
    pub initialize: fn(Message) -> Result<State, Error>,
    pub prepare: fn(&State, Message) -> Result<State, Error>,
    pub observe: fn(&State) -> Result<Message, Error>,
}

impl ModelDefinition {
    /// Exports synchronous preparation and owned immutable states over C.
    #[must_use]
    pub const fn table(&'static self) -> abi::Module {
        reovim_vtable::rv_vtable!(abi::Module {
            kind: abi::MODEL,
            declaration: ptr::from_ref(self).cast(),
            create_contract: self.initial.view(),
            read_contract: self.read.view(),
            edit_contract: self.edit.view(),
            create: Some(create),
            destroy: Some(destroy),
            call: None,
            initialize: Some(initialize),
            prepare: Some(prepare),
            observe: Some(observe),
            release: Some(release),
            drop_state: Some(drop_state),
            unload: Some(unload),
            exports: 0,
            describe: None,
            request: None,
            identity: self.identity.view(),
            dependencies: self.dependencies.as_ptr().cast(),
            dependency_count: self.dependencies.len(),
        })
    }
}

const unsafe extern "C" fn create(
    declaration: *const c_void,
    _: *const abi::Connection,
) -> *mut c_void {
    // This profile has no mutable module context; all data lives in State.
    declaration.cast_mut()
}
const unsafe extern "C" fn destroy(_: *mut c_void) {}
const unsafe extern "C" fn unload(_: *mut c_void) -> u64 {
    0
}

unsafe extern "C" fn initialize(
    context: *mut c_void,
    input: *const abi::Request,
) -> abi::Reply {
    let definition = unsafe { &*context.cast::<ModelDefinition>() };
    let input = unsafe { crate::decode_request(&*input) };
    candidate(input.and_then(|input| {
        validate(&definition.initial, &input)?;
        (definition.initialize)(input)
    }))
}
unsafe extern "C" fn prepare(
    context: *mut c_void,
    state: *const c_void,
    input: *const abi::Request,
) -> abi::Reply {
    let definition = unsafe { &*context.cast::<ModelDefinition>() };
    let state = unsafe { &*state.cast::<State>() };
    let input = unsafe { crate::decode_request(&*input) };
    candidate(input.and_then(|input| {
        validate(&definition.edit, &input)?;
        (definition.prepare)(state, input)
    }))
}
unsafe extern "C" fn observe(
    context: *mut c_void,
    state: *const c_void,
) -> abi::Reply {
    let definition = unsafe { &*context.cast::<ModelDefinition>() };
    let state = unsafe { &*state.cast::<State>() };
    observation((definition.observe)(state), &definition.read)
}
fn candidate(result: Result<State, Error>) -> abi::Reply {
    match result {
        Ok(state) => abi::Reply {
            error: 0,
            data: empty(),
            state_bytes: state.cost,
            state: Box::into_raw(Box::new(state)).cast(),
        },
        Err(error) => failure(error),
    }
}
fn observation(
    result: Result<Message, Error>,
    contract: &Identity,
) -> abi::Reply {
    let message = match result {
        Ok(message) => message,
        Err(error) => return failure(error),
    };
    // The synchronous C Model profile carries finite observation bytes.
    if !contract.matches(&message.contract) || !message.resources.is_empty() {
        return failure(Error::PROVIDER);
    }
    let len = message.data.len();
    let pointer = Box::into_raw(message.data).cast::<u8>();
    abi::Reply {
        error: 0,
        data: abi::Bytes { data: pointer, len },
        state: ptr::null_mut(),
        state_bytes: 0,
    }
}
const fn failure(error: Error) -> abi::Reply {
    abi::Reply {
        error: error.code.0,
        data: empty(),
        state: ptr::null_mut(),
        state_bytes: 0,
    }
}
const fn empty() -> abi::Bytes {
    abi::Bytes { data: ptr::null(), len: 0 }
}
fn validate(contract: &Identity, input: &Message) -> Result<(), Error> {
    if !contract.matches(&input.contract) {
        return Err(Error::UNSUPPORTED);
    }
    if !input.resources.is_empty() {
        return Err(Error::INVALID);
    }
    Ok(())
}
unsafe extern "C" fn release(_: *mut c_void, reply: *mut abi::Reply) {
    let reply = unsafe { &mut *reply };
    if !reply.data.data.is_null() {
        let bytes = ptr::slice_from_raw_parts_mut(
            reply.data.data.cast_mut(),
            reply.data.len,
        );
        drop(unsafe { Box::from_raw(bytes) });
        reply.data = empty();
    }
    if !reply.state.is_null() {
        drop(unsafe { Box::from_raw(reply.state.cast::<State>()) });
        reply.state = ptr::null_mut();
    }
}
unsafe extern "C" fn drop_state(_: *mut c_void, state: *mut c_void) {
    drop(unsafe { Box::from_raw(state.cast::<State>()) });
}
