use super::{Instance, bytes, contract, create, load_table, quiescent};
use alloc::{boxed::Box, rc::Rc};
use core::{ffi::c_void, future::ready, ptr};
use reovim_kernel_interface::{
    Contract, Error, Message, abi,
    buffer::{ModelOps, ModelProvider, State, StateReply},
};
use std::path::Path;

/// Loads a native immutable state model.
///
/// # Safety
/// The library must obey module ABI v2 ownership and immutability rules.
/// Callbacks must not unwind across the ABI.
/// Register before use; destroy only after unload readiness succeeds.
/// Its constructors and callbacks execute trusted native code.
/// # Errors
/// Rejects incompatible tables, malformed contracts or failed construction.
pub unsafe fn load_model(path: &Path) -> Result<ModelProvider, Error> {
    let (table, library) = unsafe { load_table(path)? };
    if table.kind != abi::MODEL
        || table.initialize.is_none()
        || table.prepare.is_none()
        || table.observe.is_none()
        || table.release.is_none()
        || table.drop_state.is_none()
        || table.destroy.is_none()
        || table.unload.is_none()
    {
        return Err(Error::UNSUPPORTED);
    }
    let initial = unsafe { contract(table.create_contract)? };
    let read = unsafe { contract(table.read_contract)? };
    let edit = unsafe { contract(table.edit_contract)? };
    let context = unsafe { create(&table, None)? };
    let owner =
        Rc::new(Instance { table, context, _library: Rc::clone(&library) });
    Ok(ModelProvider::new(
        owner,
        ModelOps { unload: Some(unload), initialize, prepare, observe },
        initial,
        read,
        edit,
        library,
    ))
}

struct NativeState {
    pointer: *mut c_void,
    owner: Rc<Instance>,
}
impl Drop for NativeState {
    fn drop(&mut self) {
        unsafe {
            (self.owner.table.drop_state.unwrap())(
                self.owner.context,
                self.pointer,
            );
        }
    }
}
fn request(input: &Message) -> abi::Request {
    abi::Request {
        method: abi::WRITE,
        subject: 0,
        contract: abi::Contract {
            name: abi::Bytes {
                data: input.contract.name.as_ptr(),
                len: input.contract.name.len(),
            },
            version: input.contract.version,
        },
        data: abi::Bytes { data: input.data.as_ptr(), len: input.data.len() },
        resources: ptr::null(),
        resource_count: 0,
    }
}
fn initialize(owner: &mut Rc<Instance>, input: Message) -> StateReply {
    let request = request(&input);
    let reply = unsafe {
        (owner.table.initialize.unwrap())(owner.context, &raw const request)
    };
    drop(input); // Native input views expired when the callback returned.
    Box::pin(ready(adopt(owner, reply)))
}
fn prepare(
    owner: &mut Rc<Instance>,
    state: &State,
    input: Message,
) -> StateReply {
    let result = native(owner, state).and_then(|state| {
        let request = request(&input);
        let reply = unsafe {
            (owner.table.prepare.unwrap())(
                owner.context,
                state.pointer,
                &raw const request,
            )
        };
        adopt(owner, reply)
    });
    drop(input);
    Box::pin(ready(result))
}
fn native<'a>(
    owner: &Rc<Instance>,
    state: &'a State,
) -> Result<&'a NativeState, Error> {
    let state = state.get::<NativeState>().ok_or(Error::PROVIDER)?;
    if !Rc::ptr_eq(owner, &state.owner) {
        return Err(Error::PROVIDER);
    }
    Ok(state)
}
fn adopt(owner: &Rc<Instance>, mut reply: abi::Reply) -> Result<State, Error> {
    let result = if reply.error != 0 {
        Err(Error::new(reply.error))
    } else if reply.state.is_null() {
        Err(Error::PROVIDER)
    } else {
        let value =
            NativeState { pointer: reply.state, owner: Rc::clone(owner) };
        reply.state = ptr::null_mut(); // Ownership now belongs to NativeState.
        Ok(State::new(value, reply.state_bytes))
    };
    unsafe {
        (owner.table.release.unwrap())(owner.context, &raw mut reply);
    }
    result
}
fn observe(owner: &mut Rc<Instance>, state: &State) -> Result<Message, Error> {
    let state = native(owner, state)?;
    let mut reply =
        unsafe { (owner.table.observe.unwrap())(owner.context, state.pointer) };
    let result = observe_result(owner, &reply);
    unsafe {
        (owner.table.release.unwrap())(owner.context, &raw mut reply);
    }
    result
}
fn observe_result(
    owner: &Instance,
    reply: &abi::Reply,
) -> Result<Message, Error> {
    if reply.error != 0 {
        return Err(Error::new(reply.error));
    }
    if !reply.state.is_null() {
        return Err(Error::PROVIDER);
    }
    let data = unsafe { bytes(reply.data)? };
    let contract: Contract = unsafe { contract(owner.table.read_contract)? };
    Ok(Message::new(contract, data))
}

fn unload(owner: &mut Rc<Instance>) -> Result<(), Error> {
    quiescent(owner)
}
