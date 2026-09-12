use super::{
    Instance, Library, bytes, contract, create, load_table, quiescent,
};
use alloc::{boxed::Box, rc::Rc};
use core::{ffi::c_void, future::ready, ptr};
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, abi,
    buffer::{ModelOps, ModelProvider, State, StateReply},
};
use std::path::Path;

/// Loads a native immutable state model.
///
/// # Safety
/// The library must obey module ABI 0.1.0 ownership and immutability rules.
/// Callbacks must not unwind across the ABI.
/// Register before use; destroy only after unload readiness succeeds.
/// Its constructors and callbacks execute trusted native code.
/// # Errors
/// Rejects incompatible tables, malformed contracts or failed construction.
pub unsafe fn load_model(path: &Path) -> Result<ModelProvider, Error> {
    let (table, library) = unsafe { load_table(path)? };
    super::standalone(&table)?;
    unsafe { from_table(&table, library, None) }
}

pub(super) unsafe fn from_table(
    table: &abi::Module,
    library: Rc<Library>,
    kernel: Option<&Connection>,
) -> Result<ModelProvider, Error> {
    if table.kind != abi::MODEL
        || (table.initialize.is_none() && table.initialize_async.is_none())
        || (table.prepare.is_none() && table.prepare_async.is_none())
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
    let context = unsafe { create(table, kernel, library.clone())? };
    let owner = Rc::new(Instance {
        table: *table,
        context,
        _library: Rc::clone(&library),
    });
    Ok(ModelProvider::new(
        owner,
        ModelOps {
            unload: Some(unload),
            initialize,
            prepare,
            observe,
            inspect: table.inspect.map(|_| inspect as _),
        },
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
    if let Some(initialize) = owner.table.initialize_async {
        let pending = unsafe { initialize(owner.context, &raw const request) };
        return preparation(owner, pending, owner.clone());
    }
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
    if let Some(prepare) = owner.table.prepare_async {
        let pointer = match native(owner, state) {
            Ok(state) => state.pointer,
            Err(error) => return Box::pin(ready(Err(error))),
        };
        let request = request(&input);
        let pending =
            unsafe { prepare(owner.context, pointer, &raw const request) };
        return preparation(owner, pending, Rc::new(state.clone()));
    }
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
fn preparation(
    owner: &Rc<Instance>,
    pending: abi::StatePending,
    retained: Rc<dyn core::any::Any>,
) -> StateReply {
    let owner = Rc::clone(owner);
    // Pending.release runs before its old state and executable owner expire.
    unsafe {
        reovim_sdk::import_state_reply(pending, retained, move |reply| {
            adopt(&owner, reply)
        })
    }
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
fn observe(
    owner: &mut Rc<Instance>,
    state: &State,
    input: Message,
) -> Result<Message, Error> {
    let state = native(owner, state)?;
    let mut request = request(&input);
    request.method = abi::READ;
    let mut reply = unsafe {
        (owner.table.observe.unwrap())(
            owner.context,
            state.pointer,
            &raw const request,
        )
    };
    let result = observe_result(owner, &reply);
    drop(input); // Native input views expired when the callback returned.
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

fn inspect(
    owner: &mut Rc<Instance>,
    state: State,
    input: Message,
) -> reovim_kernel_interface::provider::ProviderReply {
    let pointer = match native(owner, &state) {
        Ok(value) => value.pointer,
        Err(error) => return Box::pin(ready(Err(error))),
    };
    let mut request = request(&input);
    request.method = abi::READ;
    let pending = unsafe {
        (owner.table.inspect.unwrap())(
            owner.context,
            pointer,
            &raw const request,
        )
    };
    // The native state and executable owner outlive Pending.release.
    let future = unsafe { reovim_sdk::import_reply(pending, Rc::new(state)) };
    drop(input);
    Box::pin(async move {
        future.await.map_err(|error| match error {
            reovim_kernel_interface::RequestError::Rejected(error)
            | reovim_kernel_interface::RequestError::Failed(error) => {
                Error::new(error.code.0)
            }
            reovim_kernel_interface::RequestError::Unknown { .. } => {
                Error::PROVIDER
            }
        })
    })
}

fn unload(owner: &mut Rc<Instance>) -> Result<(), Error> {
    quiescent(owner)
}
